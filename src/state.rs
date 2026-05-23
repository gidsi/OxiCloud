use axum::extract::FromRef;
use sqlx::PgPool;
use std::sync::Arc;

use crate::config::AppConfig;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub pool: PgPool,
}

impl FromRef<AppState> for Arc<AppConfig> {
    fn from_ref(state: &AppState) -> Self {
        Arc::clone(&state.config)
    }
}
