use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, NewChangelogEntry},
        user::UserId,
    },
    ports::UnitOfWork,
};
use serde_json::json;

use crate::{
    commission::{CommissionError, CommissionPorts, CommissionResult, view::View},
    ports::WithPorts,
    transaction,
};

pub struct Command {
    pub actor_id: UserId,
    pub target_user_id: UserId,
    pub commission_id: CommissionId,
}
pub struct Output;

impl View<'_> {
    pub async fn revoke(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            target_user_id,
            commission_id,
        } = cmd;
        let mut uow = self.ports().database.begin().await?;
        let target_user = uow.users().provision(&target_user_id).await?;
        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .filter(|c| c.is_owned_by(&actor_id))
            .ok_or(CommissionError::CommissionNotFound)?;

        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::ViewGrantRevoked,
            actor_id,
            json!({
                "target_id": target_user.id,

            }),
            now,
        );

        uow.commissions()
            .revoke_view(&commission.id, &target_user.id)
            .await?;
        uow.changelog().append(&entry).await?;
        uow.commit().await?;
        Ok(Output)
    }
}
