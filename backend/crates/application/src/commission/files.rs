use domain::{
    elements::{commission::CommissionId, user::UserId},
    ports,
};
use tokio::io::AsyncRead;

use crate::commission::{
    CommissionError, CommissionPorts, CommissionResult,
    getters::{get_commission, get_user},
};

pub struct UploadFileCommand {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}
pub struct UploadFileResult;

pub async fn upload(
    command: UploadFileCommand,
    ports: CommissionPorts<'_>,
    file_stream: impl AsyncRead + Send + Unpin
) -> CommissionResult<UploadFileResult> {
    let user = get_user(command.actor_id, &ports).await?;
    let commission = get_commission(command.commission_id, &ports).await?;

    if !ports
        .commissions
        .is_participant(commission.id, user.id)
        .await
        .map_err(CommissionError::Infrastructure)?
    {
        return Err(CommissionError::NotAMember);
    }

    let uplaod = 
    todo!()
}
