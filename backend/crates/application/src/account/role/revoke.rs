use domain::{
    elements::{
        account::AccountId,
        user::{User, UserId},
    },
    ports::UnitOfWork,
};

use crate::{
    account::{AccountError, AccountPorts, AccountResult, role::Roles},
    ports::WithPorts,
    transaction,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub account_id: AccountId,
    pub target_id: UserId,
    pub actor_id: UserId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub account_id: AccountId,
    pub user_id: UserId,
}

impl Roles<'_> {
    pub async fn revoke(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            account_id,
            target_id,
            actor_id,
        } = cmd;

        let target_role = ports
            .accounts
            .role_of(&target_id, &account_id)
            .await?
            .ok_or(AccountError::UserNotFound)?;
        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|role| role.can_grant(&target_role)) // <- Elegant
            .ok_or(AccountError::IncorrectRole)?;

        let mut uow = ports.database.begin().await?;
        uow.accounts().revoke_role(&target_id, &account_id).await?;

        uow.commit().await?;
        Ok(Output {
            account_id,
            user_id: target_id,
        })
    }
}
