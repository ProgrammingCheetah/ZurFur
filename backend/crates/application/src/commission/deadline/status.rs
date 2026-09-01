use crate::{commission::deadline::Deadline, ports::WithPorts};

pub mod clear;
pub mod set;

pub struct Status<'a> {
    pub deadline: &'a Deadline<'a>,
}

impl<'a> WithPorts<'a> for Status<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.deadline.ports()
    }
}

impl<'a> Deadline<'a> {
    pub fn status(&'a self) -> Status<'a> {
        Status { deadline: self }
    }
}
