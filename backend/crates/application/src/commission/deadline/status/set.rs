use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, DeadlineStatus, NewChangelogEntry},
        user::UserId,
    },
    ports::UnitOfWork,
};
use serde_json::json;

use crate::{
    commission::{
        CommissionError, CommissionPorts, CommissionResult,
        deadline::{Deadline, DeadlineSetEventPayload, status::Status},
    },
    ports::WithPorts,
    transaction,
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub status: DeadlineStatus,
}
pub struct Output;

impl Status<'_> {
    pub async fn set(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
            status,
        } = cmd;
        match status {
            DeadlineStatus::Delayed => {}
            DeadlineStatus::Late => {
                return Err(CommissionError::InvalidStateRequested);
            }
        }
        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .ok_or(CommissionError::CommissionNotFound)?;

        if commission.deadline_status == Some(DeadlineStatus::Late) {
            return Err(CommissionError::CommissionAlreadyAtState);
        };

        if commission.deadline.is_none() {
            return Err(CommissionError::InvalidStateRequested);
        };

        let payload = DeadlineSetEventPayload {
            from: commission.deadline_status.map(|s| s.to_string()),
            to: Some(DeadlineStatus::Delayed.to_string()),
            deadline: commission.deadline,
        };
        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::Delayed,
            actor_id,
            json!({
                "from": payload.from,
                "to": payload.to,
                "deadline": payload.deadline
            }),
            now,
        );
        let mut uow = self.ports().database.begin().await?;
        uow.commissions()
            .set_deadline_status(&commission.id, Some(status))
            .await?;
        uow.changelog().append(&entry).await?;
        uow.commit().await?;

        Ok(Output)
    }
}
