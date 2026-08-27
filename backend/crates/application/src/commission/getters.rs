use domain::elements::{
    commission::{Commission, CommissionId},
    user::{User, UserId},
};

use crate::commission::{CommissionError, CommissionPorts};

pub async fn get_user(
    user_id: UserId,
    ports: &CommissionPorts<'_>,
) -> Result<User, CommissionError> {
    let Some(user) = ports
        .users
        .find_by_did(&user_id)
        .await
        .map_err(CommissionError::Infrastructure)?
    else {
        return Err(CommissionError::UserNotFound);
    };

    Ok(user)
}

pub async fn get_commission(
    commission_id: CommissionId,
    ports: &CommissionPorts<'_>,
) -> Result<Commission, CommissionError> {
    let Some(commission) = ports
        .commissions
        .find(commission_id)
        .await
        .map_err(CommissionError::Infrastructure)?
    else {
        return Err(CommissionError::CommissionNotFound);
    };

    Ok(commission)
}
