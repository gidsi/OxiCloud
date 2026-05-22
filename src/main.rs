use oxicloud::app::create_router;
use oxicloud::config::AppConfig;
use oxicloud::state::AppState;
use sqlx::postgres::PgPoolOptions;
use std::{io, sync::Arc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::from_env();

    let dev_mode = std::env::args().any(|arg| arg == "--dev");
    config
        .validate_base_url(dev_mode)
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    let state = Arc::new(AppState {
        db: pool,
        config: config.clone(),
    });

    let app = create_router(state);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port)).await?;

    println!("OxiCloud listening on {}", config.port);
    axum::serve(listener, app).await?;

    Ok(())
}
