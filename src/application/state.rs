use sqlx::PgPool;
use std::sync::Arc;

use crate::application::services::auth::AuthService;
use crate::infrastructure::repositories::postgres_user_repository::PostgresUserRepository;

pub struct AppState {
    pub db_pool: PgPool,
    pub auth_service: Arc<AuthService>,
}

impl AppState {
    pub fn new(db_pool: PgPool) -> Self {
        let user_repository = Arc::new(PostgresUserRepository::new(db_pool.clone()));
        let auth_service = Arc::new(AuthService::new(user_repository));

        Self {
            db_pool,
            auth_service,
        }
    }
}
