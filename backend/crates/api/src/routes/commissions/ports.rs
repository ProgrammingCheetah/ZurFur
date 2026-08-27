use crate::AppState;
use application::commission::CommissionPorts;

pub(crate) fn commission_ports(state: &AppState) -> CommissionPorts<'_> {
    CommissionPorts {
        users: &*state.users,
        did_minter: &*state.did_minter,
        commissions: &*state.commissions,
        database: &*state.database,
    }
}
