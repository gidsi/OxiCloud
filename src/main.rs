use oxicloud::{app, config::AppConfig, state::AppState};
use sqlx::postgres::PgPoolOptions;
use std::{env, sync::Arc};

#[tokio::main]
async fn main() {
    let config = AppConfig::from_env();

    let dev_mode = env::args().any(|arg| arg == "--dev")
        || env::var("OXICLOUD_DEV")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

    config
        .validate_base_url(dev_mode)
        .expect("Invalid OxiCloud base URL configuration");

    let pool = PgPoolOptions::new()
        .connect(&config.database_url)
        .await
        .expect("Failed to connect to database");

    let state = AppState {
        config: Arc::new(config.clone()),
        pool,
    };

    let app = app(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port))
        .await
        .expect("Failed to bind TCP listener");

    axum::serve(listener, app)
        .await
        .expect("Failed to serve OxiCloud application");
}
