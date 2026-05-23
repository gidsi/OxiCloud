use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub base_url: String,
    pub port: u16,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let default = Self::default();

        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| default.database_url.clone());

        let base_url = env::var("OXICLOUD_BASE_URL")
            .or_else(|_| env::var("BASE_URL"))
            .unwrap_or_else(|_| default.base_url.clone());

        let port = env::var("PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(default.port);

        Self {
            database_url,
            base_url,
            port,
        }
    }

    pub fn validate_base_url(&self, dev_mode: bool) -> Result<(), String> {
        let base_url = self.base_url.trim();

        if base_url.is_empty() {
            return Err("AppConfig.base_url must not be empty".to_string());
        }

        if base_url.chars().any(|ch| ch.is_control() || ch.is_whitespace()) {
            return Err(format!(
                "Invalid AppConfig.base_url `{}`. The canonical base URL must not contain whitespace or control characters.",
                self.base_url
            ));
        }

        if base_url.starts_with("https://") && has_non_empty_authority(base_url, "https://") {
            return Ok(());
        }

        if dev_mode && is_allowed_dev_base_url(base_url) {
            return Ok(());
        }

        Err(format!(
            "Invalid AppConfig.base_url `{}`. Production deployments must use an https:// canonical public URL. \
             Use --dev or OXICLOUD_DEV=true only for localhost development.",
            self.base_url
        ))
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database_url: "postgres://postgres:postgres@localhost:5432/oxicloud".to_string(),
            base_url: "http://localhost".to_string(),
            port: 8080,
        }
    }
}

fn has_non_empty_authority(base_url: &str, scheme_prefix: &str) -> bool {
    let authority_and_path = &base_url[scheme_prefix.len()..];

    !authority_and_path.is_empty()
        && !authority_and_path.starts_with('/')
        && !authority_and_path.starts_with('?')
        && !authority_and_path.starts_with('#')
}

fn is_allowed_dev_base_url(base_url: &str) -> bool {
    is_localhost_http_url(base_url)
        || is_prefixed_loopback_http_url(base_url, "http://127.0.0.1")
        || is_prefixed_loopback_http_url(base_url, "http://[::1]")
}

fn is_localhost_http_url(base_url: &str) -> bool {
    let Some(rest) = base_url.strip_prefix("http://localhost") else {
        return false;
    };

    rest.is_empty() || rest.starts_with(':') || rest.starts_with('/')
}

fn is_prefixed_loopback_http_url(base_url: &str, prefix: &str) -> bool {
    let Some(rest) = base_url.strip_prefix(prefix) else {
        return false;
    };

    rest.is_empty() || rest.starts_with(':') || rest.starts_with('/')
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn validate_base_url_accepts_https_url() {
        let config = AppConfig {
            base_url: "https://cloud.example.com".to_string(),
            ..Default::default()
        };

        assert!(config.validate_base_url(false).is_ok());
    }

    #[test]
    fn validate_base_url_rejects_http_url_without_dev_mode() {
        let config = AppConfig {
            base_url: "http://cloud.example.com".to_string(),
            ..Default::default()
        };

        assert!(config.validate_base_url(false).is_err());
    }

    #[test]
    fn validate_base_url_accepts_localhost_http_url_in_dev_mode() {
        let config = AppConfig {
            base_url: "http://localhost:8080".to_string(),
            ..Default::default()
        };

        assert!(config.validate_base_url(true).is_ok());
    }

    #[test]
    fn validate_base_url_rejects_host_header_injection_payloads() {
        let config = AppConfig {
            base_url: "https://cloud.example.com\r\nLocation: https://attacker.example".to_string(),
            ..Default::default()
        };

        assert!(config.validate_base_url(false).is_err());
    }
}
