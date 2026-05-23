use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use oxicloud::application::state::AppState;
use oxicloud::interfaces::api::router::{app_router, build_metrics_router};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/oxicloud".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    let state = Arc::new(AppState::new(pool));

    let metrics_addr =
        std::env::var("METRICS_ADDR").unwrap_or_else(|_| "127.0.0.1:9090".to_string());
    let metrics_listener = TcpListener::bind(&metrics_addr)
        .await
        .expect("Failed to bind metrics listener");

    tokio::spawn(async move {
        tracing::info!(
            "Metrics listener running on {}",
            metrics_listener
                .local_addr()
                .map(|addr| addr.to_string())
                .unwrap_or(metrics_addr)
        );

        axum::serve(metrics_listener, build_metrics_router())
            .await
            .expect("Metrics server failed");
    });

    let app = app_router(state);

    let app_addr = std::env::var("APP_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let listener = TcpListener::bind(&app_addr)
        .await
        .expect("Failed to bind application listener");

    tracing::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}
