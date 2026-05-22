use axum::{middleware::from_fn_with_state, routing::any, Router};
use std::sync::Arc;

use crate::application::state::AppState;
use crate::interfaces::api::handlers::dav::dav_handler;
use crate::interfaces::api::handlers::well_known::well_known_router;
use crate::interfaces::api::middleware::auth::auth_middleware;

pub fn app_router(state: Arc<AppState>) -> Router {
    let dav_routes = Router::new()
        .route("/{*path}", any(dav_handler))
        .route("/", any(dav_handler))
        .route_layer(from_fn_with_state(state.clone(), auth_middleware));

    Router::new()
        .nest("/.well-known", well_known_router())
        .nest("/dav", dav_routes)
        .with_state(state)
}
