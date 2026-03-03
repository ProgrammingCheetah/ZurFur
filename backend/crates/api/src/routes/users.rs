use axum::{routing::get, Router};

use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new().route("/", get(|| async { () }))
}
