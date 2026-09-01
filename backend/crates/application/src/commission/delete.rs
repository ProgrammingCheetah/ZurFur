use domain::elements::{commission::CommissionId, user::UserId};

use crate::commission::{CommissionError, CommissionResult, Commissions};

pub enum Outcome {
    Deleted,
    HasFacts,
}
pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}
pub struct Output {
    pub outcome: Outcome,
}

impl Commissions<'_> {
    pub async fn delete(&self, cmd: Command) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
        } = cmd;
        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .filter(|c| c.is_owned_by(&actor_id))
            .ok_or(CommissionError::CommissionNotFound)?;

        let mut uow = self.ports().database.begin().await?;
        if uow
            .commissions()
            .commission_has_facts(&commission.id)
            .await?
        {
            return Ok(Output {
                outcome: Outcome::HasFacts,
            });
        }
        uow.commissions().delete(&commission.id).await?;

        uow.commit().await?;
        Ok(Output {
            outcome: Outcome::Deleted,
        })
    }
}
