mod auth;
mod users;

use axum::{Router, routing::get};

use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .nest("/users", users::router())
        .nest("/auth", auth::router())
}
