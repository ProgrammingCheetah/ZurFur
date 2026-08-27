use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            ChangelogEntryKind, Commission, CommissionId, CommissionTitle, NewChangelogEntry,
        },
        maturity::Maturity,
        user::UserId,
    },
    ports::UnitOfWork,
};
use serde_json::json;

use crate::{
    commission::{CommissionError, CommissionPorts, CommissionResult},
    transaction,
};

pub struct CreateCommissionCommand {
    pub actor_id: UserId,
    pub title: CommissionTitle,
    pub maturity: Option<Maturity>,
    pub deadline: Option<DateTimeUtc>,
}
pub struct CreateCommissionResult {
    pub id: CommissionId,
}

pub async fn create(
    command: CreateCommissionCommand,
    ports: CommissionPorts<'_>,
    now: DateTimeUtc,
) -> CommissionResult<CreateCommissionResult> {
    let Some(actor) = ports
        .users
        .find_by_did(&command.actor_id)
        .await
        .map_err(CommissionError::Infrastructure)?
    else {
        return Err(CommissionError::UserNotFound);
    };
    let mut commission = Commission::create(command.title, actor.id.clone(), now, command.deadline);
    commission.maturity = command.maturity;
    let entry = NewChangelogEntry::event(
        commission.id,
        ChangelogEntryKind::Created,
        actor.id,
        json!({ "title": commission.title.as_str() }),
        now,
    );

    let result = transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.commissions().create(&commission).await?;
        uow.changelog().append(&entry).await?;
        Ok(commission)
    })
    .await
    .map(|c| CreateCommissionResult { id: c.id })
    .map_err(CommissionError::Infrastructure)?;

    Ok(result)
}
