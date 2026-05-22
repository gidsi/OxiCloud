use crate::interfaces::api::handlers::well_known::{caldav_redirect, carddav_redirect};
use crate::state::AppState;
use axum::{routing::get, Router};
use std::sync::Arc;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/.well-known/caldav",
            get(caldav_redirect).head(caldav_redirect),
        )
        .route(
            "/.well-known/carddav",
            get(carddav_redirect).head(carddav_redirect),
        )
        .with_state(state)
}
