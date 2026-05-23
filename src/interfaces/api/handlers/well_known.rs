use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::Response,
};
use std::sync::Arc;

use crate::config::AppConfig;

pub async fn redirect_to_dav(State(config): State<Arc<AppConfig>>) -> Response {
    let location = dav_redirect_location(&config.base_url);

    match HeaderValue::from_str(&location) {
        Ok(location) => Response::builder()
            .status(StatusCode::MOVED_PERMANENTLY)
            .header(header::LOCATION, location)
            .header(header::CONTENT_LENGTH, "0")
            .body(Body::empty())
            .expect("valid well-known DAV redirect response"),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_LENGTH, "0")
            .body(Body::empty())
            .expect("valid internal server error response"),
    }
}

fn dav_redirect_location(base_url: &str) -> String {
    let normalized_base_url = base_url.trim_end_matches('/');
    format!("{normalized_base_url}/dav/")
}

#[cfg(test)]
mod tests {
    use super::dav_redirect_location;

    #[test]
    fn dav_redirect_location_appends_dav_root_to_base_url() {
        assert_eq!(
            dav_redirect_location("https://cloud.example.com"),
            "https://cloud.example.com/dav/"
        );
    }

    #[test]
    fn dav_redirect_location_avoids_duplicate_slashes() {
        assert_eq!(
            dav_redirect_location("https://cloud.example.com/"),
            "https://cloud.example.com/dav/"
        );
    }

    #[test]
    fn dav_redirect_location_preserves_configured_base_path() {
        assert_eq!(
            dav_redirect_location("https://cloud.example.com/oxicloud/"),
            "https://cloud.example.com/oxicloud/dav/"
        );
    }
}
