use domain::{
    elements::{
        commission::CommissionId,
        maturity::{self, MaturityRating},
        user::UserId,
    },
    ports::UnitOfWork,
};

use crate::{
    commission::{CommissionError, CommissionPorts, CommissionResult, maturity::Maturity},
    ports::WithPorts,
    transaction,
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub maturity_rating: MaturityRating,
    pub graphic: bool,
}
pub struct Output;

impl Maturity<'_> {
    pub async fn run(&self, cmd: Command) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
            maturity_rating,
            graphic,
        } = cmd;
        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .ok_or(CommissionError::CommissionNotFound)?;
        if !commission.is_owned_by(&actor_id) {
            return Err(CommissionError::InsufficientPermissions);
        }

        let maturity = maturity::Maturity {
            graphic,
            rating: maturity_rating,
        };
        let mut uow = self.ports().database.begin().await?;
        uow.commissions()
            .set_maturity(&commission.id, maturity)
            .await?;
        uow.commit().await?;
        Ok(Output)
    }
}
