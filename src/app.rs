use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;

use crate::interfaces::api::handlers;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
}

pub fn create_app_router(state: Arc<AppState>) -> Router {
    Router::new()
        .nest("/.well-known", handlers::well_known::well_known_router())
        .nest("/dav", handlers::dav::dav_routes())
        .nest("/api/v1/auth", handlers::auth::auth_routes())
        .nest("/api/v1/users", handlers::users::users_routes())
        .with_state(state)
}
