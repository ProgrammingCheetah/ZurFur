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
    commission::{CommissionError, CommissionPorts, CommissionResult},
    transaction,
};

pub struct UnarchiveCommissionCommand {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}
pub struct UnarchiveCommissionResult {
    pub commission_id: CommissionId,
}
pub async fn unarchive(
    command: UnarchiveCommissionCommand,
    ports: CommissionPorts<'_>,
    now: DateTimeUtc,
) -> CommissionResult<UnarchiveCommissionResult> {
    let Some(user) = ports
        .users
        .find_by_did(&command.actor_id)
        .await
        .map_err(CommissionError::Infrastructure)?
    else {
        return Err(CommissionError::UserNotFound);
    };

    let Some(commission) = ports
        .commissions
        .find(command.commission_id)
        .await
        .map_err(CommissionError::Infrastructure)?
    else {
        return Err(CommissionError::CommissionNotFound);
    };

    if !commission.is_archived() {
        return Err(CommissionError::CommissionAlreadyAtState);
    }

    if user.id != commission.owner_id {
        return Err(CommissionError::InsufficientPermissions);
    }

    let entry = NewChangelogEntry::event(
        commission.id,
        ChangelogEntryKind::Unarchived,
        user.id,
        json!({ "title": commission.title.as_str() }),
        now,
    );

    transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.commissions()
            .set_archived(commission.id, Some(now))
            .await?;
        uow.changelog().append(&entry).await?;
        Ok(())
    })
    .await
    .map_err(CommissionError::Infrastructure)?;

    Ok(UnarchiveCommissionResult {
        commission_id: commission.id,
    })
}
