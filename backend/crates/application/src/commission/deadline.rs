use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            ChangelogEntryKind, Commission, CommissionId, DeadlineStatus, NewChangelogEntry,
        },
        user::UserId,
    },
    ports::UnitOfWork,
};
use serde_json::json;

use crate::{
    commission::{CommissionError, CommissionPorts, CommissionResult},
    transaction,
};

pub struct SetDeadlineCommand {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub deadline: DateTimeUtc,
}
pub struct SetDeadlineResult;

pub async fn set(
    command: SetDeadlineCommand,
    ports: CommissionPorts<'_>,
    now: DateTimeUtc,
) -> CommissionResult<SetDeadlineResult> {
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

    if !ports
        .commissions
        .is_participant(commission.id, user.id.clone())
        .await
        .map_err(CommissionError::Infrastructure)?
    {
        return Err(CommissionError::NotAMember);
    }

    if commission.deadline.is_some_and(|d| d == now) {
        return Err(CommissionError::CommissionAlreadyAtState);
    }

    let kind = match (commission.deadline, command.deadline) {
        (Some(old), new) if new > old => ChangelogEntryKind::DeadlineExtended,
        _ => ChangelogEntryKind::DeadlineSet,
    };

    let entry = NewChangelogEntry::event(
        commission.id,
        kind,
        user.id,
        json!({ "from": commission.deadline, "to": command.deadline }),
        now,
    );

    transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.commissions()
            .set_deadline(commission.id, Some(command.deadline))
            .await?;
        uow.changelog().append(&entry).await?;
        Ok(())
    })
    .await
    .map_err(CommissionError::Infrastructure)?;
    Ok(SetDeadlineResult)
}

pub struct ClearDeadlineCommand {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}

pub struct ClearDeadlineResult;

pub async fn clear(
    command: ClearDeadlineCommand,
    ports: CommissionPorts<'_>,
    now: DateTimeUtc,
) -> CommissionResult<ClearDeadlineResult> {
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

    if !ports
        .commissions
        .is_participant(commission.id, user.id.clone())
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
        user.id,
        json!({ "from": commission.deadline, "to": None as Option<DateTimeUtc>}),
        now,
    );

    transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.commissions().set_deadline(commission.id, None).await?;
        uow.changelog().append(&entry).await?;
        Ok(())
    })
    .await
    .map_err(CommissionError::Infrastructure)?;

    Ok(ClearDeadlineResult)
}

pub struct SetDeadlineStatusCommand {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub status: DeadlineStatus,
}
pub struct SetDeadlineStatusResult;

pub async fn set_status(
    command: SetDeadlineStatusCommand,
    ports: CommissionPorts<'_>,
    now: DateTimeUtc,
) -> CommissionResult<SetDeadlineStatusResult> {
    match command.status {
        DeadlineStatus::Delayed => {}
        DeadlineStatus::Late => {
            return Err(CommissionError::InvalidStateRequested);
        }
    }
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
        user.id,
        json!({
            "from": payload.from,
            "to": payload.to,
            "deadline": payload.deadline
        }),
        now,
    );

    transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.commissions()
            .set_deadline_status(commission.id, Some(command.status))
            .await?;
        uow.changelog().append(&entry).await?;
        Ok(())
    })
    .await
    .map_err(CommissionError::Infrastructure)?;

    Ok(SetDeadlineStatusResult)
}

pub struct ClearDeadlineStatusCommand {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}
pub struct ClearDeadlineStatusResult;

pub async fn clear_status(
    command: ClearDeadlineStatusCommand,
    ports: CommissionPorts<'_>,
    now: DateTimeUtc,
) -> CommissionResult<ClearDeadlineStatusResult> {
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

    if !ports
        .commissions
        .is_participant(commission.id, user.id.clone())
        .await
        .map_err(CommissionError::Infrastructure)?
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
        user.id,
        json!({
            "from": payload.from,
            "to": payload.to,
            "deadline": payload.deadline
        }),
        now,
    );

    transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.commissions()
            .set_deadline_status(commission.id, None)
            .await?;
        uow.changelog().append(&entry).await?;

        Ok(())
    })
    .await
    .map_err(CommissionError::Infrastructure)?;

    Ok(ClearDeadlineStatusResult)
}

pub struct DeadlineSetEventPayload {
    pub from: Option<String>,
    pub to: Option<String>,
    pub deadline: Option<DateTimeUtc>,
}
