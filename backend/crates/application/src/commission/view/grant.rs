use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, GrantLevel, NewChangelogEntry},
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
    pub level: GrantLevel,
}
pub struct Output;

impl View<'_> {
    pub async fn grant(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            target_user_id,
            commission_id,
            level,
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
            ChangelogEntryKind::ViewGrantIssued,
            actor_id,
            json!({
                "commission_id": commission.id,
                "user": target_user.id,
                "level": level.to_string()
            }),
            now,
        );

        let mut uow = self.ports().database.begin().await?;
        uow.commissions()
            .grant_view(&commission.id, &target_user.id, level)
            .await?;
        uow.changelog().append(&entry).await?;
        uow.commit().await?;

        Ok(Output)
    }
}
