//! `zurfur account delete` must render exactly like `DELETE /accounts/{id}`
//! (ZMVP-205 slice 5). Both drivers call the one use case
//! (`application::account::delete_account`) and project its
//! `DeleteAccountResult`; the CLI cannot name the generated
//! `DeleteAccountResponse` (it lives inside `api`, behind axum), so its
//! `Deleted` is a hand copy. This test keeps the two projections identical —
//! for BOTH outcomes, so neither driver can drift on the `soft`/`hard`
//! spelling — until the contract moves to a leaf crate (DD 40992770 D11); the
//! same guard `create_account_parity.rs` gives `POST /accounts`.

use api::generated::DeleteAccountResponse;
use application::account::{DeleteAccountResult, DeleteOutcome};
use cli::commands::account::Deleted;

#[test]
fn account_delete_renders_exactly_like_delete_accounts() {
    for outcome in [DeleteOutcome::Soft, DeleteOutcome::Hard] {
        let result = DeleteAccountResult { outcome };
        let http = serde_json::to_value(DeleteAccountResponse::from(result)).unwrap();
        let terminal = serde_json::to_value(Deleted::from(result)).unwrap();
        assert_eq!(terminal, http, "the two projections of {outcome:?} differ");
    }
}
