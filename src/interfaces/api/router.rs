use axum::{middleware, Router};
use std::sync::Arc;

use crate::application::state::AppState;
use crate::interfaces::api::{handlers, middlewares};

use middlewares::auth::require_auth;

pub fn app_router(state: Arc<AppState>) -> Router {
    let dav_routes = Router::new()
        .nest("/dav", handlers::dav::dav_routes())
        .nest("/remote.php/webdav", handlers::dav::dav_routes())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_auth,
        ));

    Router::new()
        .nest(
            "/.well-known",
            handlers::well_known::well_known_router::<Arc<AppState>>(),
        )
        .merge(dav_routes)
        .with_state(state)
}
