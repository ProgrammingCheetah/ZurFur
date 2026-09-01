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
    commission::{CommissionError, CommissionPorts, CommissionResult, deadline::Deadline},
    ports::WithPorts,
    transaction,
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub deadline: DateTimeUtc,
}
pub struct Output;

impl Deadline<'_> {
    pub async fn set(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
            deadline,
        } = cmd;
        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .ok_or(CommissionError::UserNotFound)?;

        if !ports
            .commissions
            .is_participant(&commission.id, &actor_id)
            .await?
        {
            return Err(CommissionError::NotAMember);
        }

        if commission.deadline.is_some_and(|d| d == now) {
            return Err(CommissionError::CommissionAlreadyAtState);
        }

        let kind = match (commission.deadline, deadline) {
            (Some(old), new) if new > old => ChangelogEntryKind::DeadlineExtended,
            _ => ChangelogEntryKind::DeadlineSet,
        };

        let entry = NewChangelogEntry::event(
            commission.id,
            kind,
            actor_id,
            json!({ "from": commission.deadline, "to": deadline }),
            now,
        );

        let mut uow = self.ports().database.begin().await?;
        uow.commissions()
            .set_deadline(&commission.id, Some(deadline))
            .await?;
        uow.changelog().append(&entry).await?;
        uow.commit().await?;
        Ok(Output)
    }
}
