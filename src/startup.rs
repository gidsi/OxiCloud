use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::interfaces::api::router::app_router;
use crate::telemetry::memory::initialize_process_memory_metrics;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
}

pub struct Application {
    port: u16,
    router: Router,
}

impl Application {
    pub async fn build() -> Result<Self, std::io::Error> {
        let db_pool = PgPool::connect("postgres://postgres:password@localhost:5432/oxicloud")
            .await
            .map_err(|error| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("failed to connect to Postgres: {error}"),
                )
            })?;

        initialize_process_memory_metrics().await;

        let state = Arc::new(AppState { db_pool });
        let router = app_router(state);

        Ok(Self { port: 8000, router })
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;
        tracing::info!("listening on {}", listener.local_addr().unwrap());
        axum::serve(listener, self.router).await
    }
}
