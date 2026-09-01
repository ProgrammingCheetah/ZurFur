use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId},
        user::UserId,
    },
};

use crate::{
    commission::{CommissionError, CommissionResult, changelog::Changelog},
    ports::WithPorts,
};
pub struct Query {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}

pub struct ChangelogEntry {
    pub seq: i64,
    pub kind: ChangelogEntryKind,
    pub actor_id: Option<UserId>,
    pub payload: serde_json::Value,
    pub note: Option<String>,
    pub created_at: DateTimeUtc,
}
pub struct Output {
    pub entries: Vec<ChangelogEntry>,
}

impl Changelog<'_> {
    pub async fn read(&self, query: Query) -> CommissionResult<Output> {
        let ports = self.ports();
        let Query {
            actor_id,
            commission_id,
        } = query;
        if !ports
            .commissions
            .is_participant(&commission_id, &actor_id)
            .await?
        {
            // The closed door: a non-participant is told the commission does
            // not exist, never that it exists but is forbidden — a 403 here
            // would confirm a private commission to any signed-in stranger who
            // guessed (or was handed) its id.
            return Err(CommissionError::NotAMember);
        }

        Ok(Output {
            entries: ports
                .changelog
                .entries(&commission_id)
                .await?
                .into_iter()
                .enumerate()
                .map(|(seq, entry)| ChangelogEntry {
                    seq: seq as i64,
                    actor_id: entry.actor_id,
                    kind: entry.kind,
                    payload: entry.payload,
                    note: entry.note,
                    created_at: entry.created_at,
                })
                .collect(),
        })
    }
}
