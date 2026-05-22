use axum::{
    extract::{Request, State},
    http::{
        header::{AUTHORIZATION, WWW_AUTHENTICATE},
        HeaderValue, StatusCode,
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

use crate::application::state::AppState;

const BASIC_AUTH_CHALLENGE: &str = r#"Basic realm="OxiCloud", charset="UTF-8""#;

pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    let Some(token) = token else {
        return unauthorized_response();
    };

    match state.auth_service.validate_session(token).await {
        Ok(user) => {
            req.extensions_mut().insert(user);
            next.run(req).await
        }
        Err(_) => unauthorized_response(),
    }
}

fn unauthorized_response() -> Response {
    let mut response = StatusCode::UNAUTHORIZED.into_response();
    response.headers_mut().insert(
        WWW_AUTHENTICATE,
        HeaderValue::from_static(BASIC_AUTH_CHALLENGE),
    );
    response
}
