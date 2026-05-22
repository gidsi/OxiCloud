use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub port: u16,
    pub base_url: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:password@localhost:5432/oxicloud".to_string()
            }),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .unwrap_or(8080),
            base_url: env::var("BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        Self::default()
    }

    pub fn validate_base_url(&self, allow_insecure: bool) -> Result<(), String> {
        if self.base_url.trim().is_empty() {
            return Err("BASE_URL must not be empty".to_string());
        }

        if self.base_url.trim() != self.base_url {
            return Err("BASE_URL must not contain leading or trailing whitespace".to_string());
        }

        if !allow_insecure && !self.base_url.starts_with("https://") {
            return Err(format!(
                "BASE_URL must start with https:// for Apple-compatible DAV discovery; got '{}'. Use --dev only for local insecure development.",
                self.base_url
            ));
        }

        Ok(())
    }
}
