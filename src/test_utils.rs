use axum::body::Body;
use axum::http::Request;
use axum::Router;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;

use crate::app::{create_app_router, AppState};

pub struct TestState {
    pub pool: PgPool,
    pub app: Router,
}

impl TestState {
    pub async fn new() -> Self {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/oxicloud_test".to_string()
        });

        let pool = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to test database");

        let state = Arc::new(AppState {
            db_pool: pool.clone(),
        });

        let app = create_app_router(state);

        Self { pool, app }
    }

    pub fn new_dummy() -> Arc<AppState> {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy_with(
                PgConnectOptions::new()
                    .host("127.0.0.1")
                    .port(1)
                    .username("oxicloud_dummy")
                    .password("oxicloud_dummy")
                    .database("oxicloud_dummy"),
            );

        Arc::new(AppState { db_pool: pool })
    }

    pub async fn execute_request(&mut self, req: Request<Body>) -> axum::http::Response<Body> {
        self.app
            .clone()
            .oneshot(req)
            .await
            .expect("Failed to execute request")
    }
}
