use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, ElementId, SeatInvitation, SeatInvitationId, element::SeatId},
        invitation::InvitationState,
        user::UserId,
    },
    ports::UnitOfWork,
};

use crate::{
    account::AccountPorts,
    commission::{CommissionError, CommissionPorts, CommissionResult, invitations::Invitations},
    ports::WithPorts,
    transaction,
};

pub struct Command {
    pub inviting_actor_id: UserId,
    pub invited_actor_id: UserId,
    pub commission_id: CommissionId,
    pub seat_id: SeatId,
}

pub enum Output {
    Created(InvitationOutput),
    PreExisting(InvitationOutput),
}
pub struct InvitationOutput {
    pub invitation_id: SeatInvitationId,
    pub invitation_state: InvitationState,
    pub commission_id: CommissionId,
    pub seat_id: SeatId,
    pub invited_user_id: UserId,
}

impl Invitations<'_> {
    pub async fn issue(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            inviting_actor_id: _,
            invited_actor_id,
            commission_id,
            seat_id,
        } = cmd;
        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .ok_or(CommissionError::CommissionNotFound)?;

        if !commission.is_owned_by(&invited_actor_id) {
            return Err(CommissionError::InsufficientPermissions);
        }

        // Quick note: Before, users had their own ID. Since we changed it to DID, inviting a user
        // Is the semantic equivalent of using their DID every time.
        // An invited user MUST exist, therefore; always available.
        let mut uow = self.ports().database.begin().await?;
        // FIXME: Is a DID only a user's? How do we differentiate between them and accounts?
        let target_user = uow.users().provision(&invited_actor_id).await?;
        let seats = ports.commissions.seats(&commission.id).await?;

        let seat = seats
            .iter()
            .find(|seat| seat.id == seat_id && seat.is_vacant())
            .ok_or(CommissionError::SeatNotFound)?;

        if let Some(invitation) = ports
            .commissions
            .find_pending_seat_invitation(&commission.id, &seat.id, &target_user.id)
            .await?
        {
            return Ok(Output::PreExisting(InvitationOutput {
                commission_id: commission.id,
                invitation_id: invitation.id,
                invitation_state: invitation.state,
                invited_user_id: target_user.id,
                seat_id: seat.id,
            }));
        }

        let invitation = SeatInvitation::issue(
            commission.id,
            seat.id,
            target_user.id.clone(),
            invited_actor_id.clone(),
            now,
        );
        let minted = invitation.id;

        uow.commissions()
            .create_seat_invitation(&invitation)
            .await?;

        let output = match ports
            .commissions
            .find_pending_seat_invitation(&commission.id, &seat.id, &target_user.id)
            .await?
        {
            Some(stored) => {
                let invitation = InvitationOutput {
                    invitation_id: stored.id,
                    invitation_state: stored.state,
                    commission_id: commission.id,
                    seat_id: seat.id,
                    invited_user_id: target_user.id,
                };
                if stored.id == minted {
                    Output::Created(invitation)
                } else {
                    Output::PreExisting(invitation)
                }
            }
            None => Output::Created(InvitationOutput {
                invitation_id: minted,
                invitation_state: InvitationState::Pending,
                commission_id: commission.id,
                seat_id: seat.id,
                invited_user_id: target_user.id,
            }),
        };
        uow.commit().await?;
        Ok(output)
    }
}
