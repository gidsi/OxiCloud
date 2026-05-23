use axum::{
    extract::Request,
    http::{header, HeaderMap, Method},
    middleware::Next,
    response::Response,
};

use crate::error::AppError;

const CSRF_COOKIE: &str = "csrf_token";
const CSRF_HEADER: &str = "x-csrf-token";

pub async fn csrf_middleware(request: Request, next: Next) -> Result<Response, AppError> {
    if should_skip_csrf(request.method()) {
        return Ok(next.run(request).await);
    }

    let cookie_token = extract_cookie_value(request.headers(), CSRF_COOKIE);
    let header_token = request
        .headers()
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok());

    match (cookie_token.as_deref(), header_token) {
        (Some(cookie), Some(header)) if !cookie.is_empty() && cookie == header => {
            Ok(next.run(request).await)
        }
        _ => Err(AppError::Forbidden(
            "CSRF token missing or invalid".to_string(),
        )),
    }
}

fn should_skip_csrf(method: &Method) -> bool {
    if matches!(method, &Method::GET | &Method::HEAD | &Method::OPTIONS) {
        return true;
    }

    matches!(
        method.as_str(),
        "PROPFIND"
            | "PROPPATCH"
            | "MKCOL"
            | "COPY"
            | "MOVE"
            | "LOCK"
            | "UNLOCK"
            | "REPORT"
            | "MKCALENDAR"
    )
}

fn extract_cookie_value(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;

    cookie_header.split(';').find_map(|cookie| {
        let (name, value) = cookie.trim().split_once('=')?;

        if name == cookie_name {
            Some(value.to_string())
        } else {
            None
        }
    })
}
