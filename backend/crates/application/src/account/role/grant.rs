use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::AccountId, commission::NewChangelogEntry, role::Role, user::UserId,
        user_account::UserAccount,
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
    pub role: Role,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub account_id: AccountId,
    pub role: Role,
    pub user_id: UserId,
}
impl Roles<'_> {
    pub async fn grant(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            account_id,
            target_id,
            actor_id,
            role,
        } = cmd;
        if !matches!(role, Role::Owner) {
            return Err(AccountError::IncorrectTransferOfAccount);
        };

        let mut uow = self.ports().database.begin().await?;

        let target = uow.users().provision(&target_id).await?;

        let actor_role = ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|r| r.can_grant(&role))
            .ok_or(AccountError::IncorrectRole)?;

        uow.users().provision(&target.id).await?;

        if let Some(current_role) = ports.accounts.role_of(&target.id, &account_id).await?
            && !actor_role.can_grant(&current_role)
        {
            return Err(AccountError::IncorrectRole);
        }

        let member = UserAccount {
            user_id: target.id.clone(),
            account_id: account_id.clone(),
            alias: None,
            role: role.clone(),
        };

        uow.accounts().grant_role(&member).await?;

        uow.commit().await?;
        Ok(Output {
            account_id,
            role,
            user_id: target.id.clone(),
        })
    }
}
