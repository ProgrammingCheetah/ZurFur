use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, element::SeatId},
        user::UserId,
    },
    ports::UnitOfWork,
};

use crate::{
    commission::{CommissionError, CommissionPorts, CommissionResult, invitations::Invitations},
    ports::WithPorts,
    transaction,
};

pub struct Command {
    pub actor_id: UserId,
    pub target_id: UserId,
    pub commission_id: CommissionId,
    pub seat_id: SeatId,
}
pub struct Output {
    pub commission_id: CommissionId,
    pub seat_id: SeatId,
    pub user_id: UserId,
}

impl Invitations<'_> {
    pub async fn revoke(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            target_id,
            commission_id,
            seat_id,
        } = cmd;

        ports
            .commissions
            .find(&commission_id)
            .await?
            .filter(|c| c.is_owned_by(&actor_id))
            .ok_or(CommissionError::InsufficientPermissions)?;

        ports
            .commissions
            .seats(&commission_id)
            .await?
            .into_iter()
            .find(|seat| seat.id == seat_id)
            .ok_or(CommissionError::SeatNotFound)?;

        let Some(mut invitation) = ports
            .commissions
            .find_pending_seat_invitation(&commission_id, &seat_id, &target_id)
            .await?
        else {
            return Ok(Output {
                commission_id,
                seat_id,
                user_id: target_id,
            });
        };

        invitation
            .revoke(now)
            .map_err(|_| CommissionError::InvalidStateRequested)?;
        let mut uow = self.ports().database.begin().await?;
        uow.commissions()
            .revoke_seat_invitation(&invitation.id)
            .await?;

        Ok(Output {
            commission_id,
            seat_id,
            user_id: target_id,
        })
    }
}
