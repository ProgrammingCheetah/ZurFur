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
}
pub struct Output;

impl Status<'_> {
    pub async fn clear(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
        } = cmd;
        let ports = self.ports();
        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .ok_or(CommissionError::CommissionNotFound)?;

        if !ports
            .commissions
            .is_participant(&commission.id, &actor_id)
            .await?
        {
            return Err(CommissionError::NotAMember);
        }

        if commission.deadline_status.is_none() {
            return Err(CommissionError::CommissionAlreadyAtState);
        }

        let payload = DeadlineSetEventPayload {
            deadline: commission.deadline,
            from: commission.deadline_status.map(|ds| ds.to_string()),
            to: None,
        };

        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::DeadlineSet,
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
            .set_deadline_status(&commission.id, None)
            .await?;
        uow.changelog().append(&entry).await?;

        uow.commit().await?;
        Ok(Output)
    }
}
