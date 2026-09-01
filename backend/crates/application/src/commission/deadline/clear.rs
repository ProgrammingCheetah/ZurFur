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
}

pub struct Output;

impl Deadline<'_> {
    pub async fn clear(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
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

        if !ports
            .commissions
            .is_participant(&commission.id, &actor_id)
            .await
            .map_err(CommissionError::Infrastructure)?
        {
            return Err(CommissionError::NotAMember);
        }

        if commission.deadline.is_none() {
            return Err(CommissionError::CommissionAlreadyAtState);
        }

        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::DeadlineSet,
            actor_id,
            json!({ "from": commission.deadline, "to": None as Option<DateTimeUtc>}),
            now,
        );
        let mut uow = self.ports().database.begin().await?;
        uow.commissions().set_deadline(&commission.id, None).await?;
        uow.changelog().append(&entry).await?;

        uow.commit().await?;
        Ok(Output)
    }
}
