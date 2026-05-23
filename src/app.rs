use std::sync::Arc;

use axum::{middleware, Router};
use sqlx::PgPool;

use crate::interfaces::api::handlers;
use crate::interfaces::api::handlers::well_known::well_known_router;
use crate::interfaces::api::middlewares::auth::auth_middleware;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

pub fn build_app(pool: PgPool) -> Router {
    let state = Arc::new(AppState { db: pool });

    let api_routes = Router::new()
        .nest("/users", handlers::users::router())
        .nest("/files", handlers::files::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let dav_routes = Router::new()
        .nest("/dav", handlers::webdav::router())
        .nest("/webdav", handlers::webdav::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .nest("/.well-known", well_known_router())
        .nest("/api", api_routes)
        .merge(dav_routes)
        .with_state(state)
}

pub fn create_router(pool: PgPool) -> Router {
    build_app(pool)
}
