use domain::{
    elements::{commission::CommissionId, user::UserId},
    ports::UnitOfWork,
};

use crate::{
    commission::{
        CommissionError, CommissionPorts, CommissionResult,
        getters::{get_commission, get_user},
    },
    transaction,
};

pub enum DeleteOutcome {
    Deleted,
    HasFacts,
}
pub struct DeleteCommissionCommand {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}
pub struct DeleteCommissionResult {
    pub outcome: DeleteOutcome,
}

pub async fn delete(
    command: DeleteCommissionCommand,
    ports: CommissionPorts<'_>,
) -> CommissionResult<DeleteCommissionResult> {
    let user = get_user(command.actor_id, &ports).await?;
    let commission = get_commission(command.commission_id, &ports).await?;

    if !commission.is_owned_by(&user.id) {
        return Err(CommissionError::InsufficientPermissions);
    };

    let outcome = transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        if uow
            .commissions()
            .commission_has_facts(commission.id)
            .await?
        {
            return Ok(DeleteOutcome::HasFacts);
        }
        uow.commissions().delete(commission.id).await?;
        Ok(DeleteOutcome::Deleted)
    })
    .await
    .map_err(CommissionError::Infrastructure)?;

    Ok(DeleteCommissionResult { outcome })
}
