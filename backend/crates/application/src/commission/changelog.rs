use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId},
        user::UserId,
    },
};

use crate::commission::{CommissionError, CommissionPorts, CommissionResult};
pub struct ReadChangelogQuery {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}

struct ChangelogEntryBody {
    pub seq: i64,
    pub kind: ChangelogEntryKind,
    pub actor_id: Option<UserId>,
    pub payload: serde_json::Value,
    pub note: Option<String>,
    pub created_at: DateTimeUtc,
}
pub struct ReadChangelogResult {
    pub entries: Vec<ChangelogEntryBody>,
}

pub async fn read(
    query: ReadChangelogQuery,
    ports: CommissionPorts<'_>,
) -> CommissionResult<ReadChangelogResult> {
    let Some(user) = ports
        .users
        .find_by_did(&query.actor_id)
        .await
        .map_err(CommissionError::Infrastructure)?
    else {
        return Err(CommissionError::UserNotFound);
    };

    let Some(commission) = ports
        .commissions
        .find(query.commission_id)
        .await
        .map_err(CommissionError::Infrastructure)?
    else {
        return Err(CommissionError::NotAMember);
    };

    if !ports
        .commissions
        .is_participant(commission.id.clone(), user.id)
        .await
        .map_err(CommissionError::Infrastructure)?
    {
        return Err(CommissionError::InsufficientPermissions);
    }

    let entries: Vec<ChangelogEntryBody> = ports
        .changelog
        .entries(commission.id)
        .await
        .map_err(CommissionError::Infrastructure)?
        .into_iter()
        .enumerate()
        .map(|(seq, entry)| ChangelogEntryBody {
            seq: seq as i64,
            actor_id: entry.actor_id,
            kind: entry.kind,
            payload: entry.payload,
            note: entry.note,
            created_at: entry.created_at,
        })
        .collect();
    Ok(ReadChangelogResult { entries })
}
