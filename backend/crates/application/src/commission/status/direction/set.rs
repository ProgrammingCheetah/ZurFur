use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, DirectionStatus, NewChangelogEntry},
        user::UserId,
    },
    ports::UnitOfWork,
};
use serde_json::json;

use crate::{
    commission::{
        CommissionError, CommissionPorts, CommissionResult, status::direction::Direction,
    },
    ports::WithPorts,
    transaction,
};

pub struct Command {
    pub user_id: UserId,
    pub commission_id: CommissionId,
    pub direction: Option<DirectionStatus>,
}
pub struct Output;

impl Direction<'_> {
    pub async fn set(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            user_id,
            commission_id,
            direction,
        } = cmd;
        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .filter(|c| c.direction_status != direction)
            .ok_or(CommissionError::CommissionNotFound)?;

        if ports
            .commissions
            .is_participant(&commission.id, &user_id)
            .await?
        {
            return Err(CommissionError::NotAMember);
        }

        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::StatusChanged,
            user_id,
            json!({
                "from": commission.direction_status.map(|s| s.to_string()),
                "to": direction.map(|s| s.to_string())
            }),
            now,
        );

        let mut uow = self.ports().database.begin().await?;
        uow.commissions()
            .set_direction_status(&commission.id, direction)
            .await?;
        uow.changelog().append(&entry).await?;
        uow.commit().await?;
        Ok(Output)
    }
}
