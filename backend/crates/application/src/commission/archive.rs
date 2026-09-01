use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, Commission, CommissionId, NewChangelogEntry},
        user::UserId,
    },
    ports::UnitOfWork,
};
use serde_json::json;

use crate::{
    commission::{CommissionError, CommissionPorts, CommissionResult, Commissions},
    transaction,
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}
pub struct Outcome;

impl Commissions<'_> {
    pub async fn archive(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Outcome> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
        } = cmd;
        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .ok_or(CommissionError::CommissionNotFound)?;

        if commission.is_archived() {
            return Err(CommissionError::CommissionAlreadyAtState);
        }

        if actor_id != commission.owner_id {
            return Err(CommissionError::InsufficientPermissions);
        }

        // FIXME: This needs to be called using ports.changelog().event().new()
        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::Archived,
            actor_id,
            json!({ "title": commission.title.as_str() }),
            now,
        );

        let mut uow = self.ports().database.begin().await?;
        uow.commissions()
            .set_archived(&commission.id, Some(now))
            .await?;
        uow.changelog().append(&entry).await?;
        uow.commit().await?;
        Ok(Outcome)
    }
}
