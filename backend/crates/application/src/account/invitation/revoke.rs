use domain::{
    elements::{account::AccountId, user::UserId},
    ports::UnitOfWork,
};

use crate::{
    account::{AccountError, AccountPorts, AccountResult, invitation::Invitations},
    ports::WithPorts,
    transaction,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub account_id: AccountId,
    pub actor_id: UserId,
    pub target_id: UserId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output;

impl Invitations<'_> {
    pub async fn revoke(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            account_id,
            actor_id,
            target_id,
        } = cmd;

        if ports
            .accounts
            .role_of(&target_id, &account_id)
            .await?
            .is_some()
        {
            return Err(AccountError::AlreadyMember);
        };

        let invitation = ports
            .accounts
            .find_pending_invitation(&account_id, &target_id)
            .await?
            .ok_or(AccountError::NoPendingInvitation)?;

        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|r| r.can_grant(&invitation.role))
            .ok_or(AccountError::NotAMember)?;

        let mut uow = self.ports().database.begin().await?;
        uow.accounts().revoke_invitation(&invitation.id).await?;
        uow.commit().await?;
        Ok(Output)
    }
}
