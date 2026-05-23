use axum::{routing::any, Router};

use crate::state::AppState;

use super::handlers;

pub fn app(state: AppState) -> Router {
    Router::new()
        .route(
            "/.well-known/caldav",
            any(handlers::well_known::redirect_to_dav),
        )
        .route(
            "/.well-known/carddav",
            any(handlers::well_known::redirect_to_dav),
        )
        .with_state(state)
}
