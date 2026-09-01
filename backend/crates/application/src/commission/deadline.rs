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
    commission::{CommissionError, CommissionPorts, CommissionResult, Commissions},
    ports::WithPorts,
    transaction,
};

pub mod clear;
pub mod set;
pub mod status;

pub(super) struct DeadlineSetEventPayload {
    pub from: Option<String>,
    pub to: Option<String>,
    pub deadline: Option<DateTimeUtc>,
}

pub struct Deadline<'a> {
    commissions: &'a Commissions<'a>,
}

impl<'a> WithPorts<'a> for Deadline<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.commissions.ports()
    }
}

impl<'a> Commissions<'a> {
    pub fn deadline(&'a self) -> Deadline<'a> {
        Deadline { commissions: self }
    }
}
