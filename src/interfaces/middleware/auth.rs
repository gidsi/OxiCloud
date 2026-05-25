use axum::{
    extract::{FromRequestParts, Request, State},
    http::{HeaderMap, StatusCode, header, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::Engine;
use std::sync::Arc;
use uuid::Uuid;

use crate::application::dtos::dav_auth_failure_dto::CreateDavAuthFailureDto;
pub use crate::application::dtos::user_dto::CurrentUser;
use crate::application::ports::auth_ports::TokenServicePort;
use crate::application::ports::dav_auth_failure_ports::DavAuthFailureStoragePort;
use crate::common::di::AppState;
use crate::common::errors::DomainError;
use crate::interfaces::api::cookie_auth::{ACCESS_COOKIE, extract_cookie_value};
use crate::interfaces::errors::AppError;
use crate::interfaces::middleware::rate_limit::extract_client_ip;

/// Marker inserted into request extensions when the user was authenticated
/// via the `oxicloud_access` HttpOnly cookie rather than a Bearer/Basic header.
/// The CSRF middleware uses this to decide whether CSRF validation is required.
#[derive(Clone, Copy, Debug)]
pub struct CookieAuthenticated;

// Newtype over Arc<CurrentUser> for zero-allocation extraction.
// `Deref<Target = CurrentUser>` lets handlers access `.id`, `.username`,
// `.email`, `.role` transparently — no signature changes needed.
#[derive(Clone, Debug)]
pub struct AuthUser(pub Arc<CurrentUser>);

impl std::ops::Deref for AuthUser {
    type Target = CurrentUser;

    #[inline]
    fn deref(&self) -> &CurrentUser {
        &self.0
    }
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Arc<CurrentUser>>()
            .cloned()
            .map(AuthUser)
            .ok_or_else(|| AppError::unauthorized("Authentication required"))
    }
}

/// Reusable extractor that gets the user_id of the authenticated user.
/// Automatically extracted from the `CurrentUser` inserted by the auth middleware.
#[derive(Clone, Copy, Debug)]
pub struct CurrentUserId(pub Uuid);

impl<S> FromRequestParts<S> for CurrentUserId
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        Ok(CurrentUserId(user.id))
    }
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthMiddlewareError> {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let client_ip = extract_client_ip(&request);

    if let Some(token) = bearer_token(&headers) {
        match authenticate_bearer(&state, token) {
            Ok(user) => {
                request.extensions_mut().insert(Arc::new(user));
                return Ok(next.run(request).await);
            }
            Err(err) => {
                tracing::warn!(path = %path, "Bearer token authentication failed: {}", err);
                return Err(AuthMiddlewareError::Unauthorized);
            }
        }
    }

    if let Some(token) = extract_cookie_value(&headers, ACCESS_COOKIE) {
        match authenticate_bearer(&state, &token) {
            Ok(user) => {
                request.extensions_mut().insert(Arc::new(user));
                request.extensions_mut().insert(CookieAuthenticated);
                return Ok(next.run(request).await);
            }
            Err(err) => {
                tracing::warn!(path = %path, "Cookie authentication failed: {}", err);
                return Err(AuthMiddlewareError::Unauthorized);
            }
        }
    }

    if let Some((username, password)) = basic_credentials(&headers) {
        match authenticate_basic(&state, &username, &password).await {
            Ok(user) => {
                if let Some(auth_svc) = state.auth_service.as_ref() {
                    auth_svc.login_lockout.record_success(&username);
                }
                request.extensions_mut().insert(Arc::new(user));
                return Ok(next.run(request).await);
            }
            Err(err) => {
                if let Some(auth_svc) = state.auth_service.as_ref() {
                    auth_svc.login_lockout.record_failure(&username);
                }
                record_dav_auth_failure(
                    &state,
                    CreateDavAuthFailureDto {
                        client_ip,
                        username,
                        method,
                        path,
                        user_agent,
                        reason: err.to_string(),
                        auth_scheme: "Basic".to_string(),
                        protocol: "DAV".to_string(),
                    },
                )
                .await;
                return Err(AuthMiddlewareError::BasicUnauthorized);
            }
        }
    }

    Err(AuthMiddlewareError::Unauthorized)
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    if scheme.eq_ignore_ascii_case("Bearer") && !token.trim().is_empty() {
        Some(token.trim())
    } else {
        None
    }
}

fn basic_credentials(headers: &HeaderMap) -> Option<(String, String)> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, encoded) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Basic") {
        return None;
    }

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let (username, password) = decoded.split_once(':')?;

    Some((username.to_string(), password.to_string()))
}

fn authenticate_bearer(state: &AppState, token: &str) -> Result<CurrentUser, DomainError> {
    let auth_service = state.auth_service.as_ref().ok_or_else(|| {
        DomainError::internal_error("Auth", "Authentication service is unavailable")
    })?;
    let claims = auth_service.token_service.validate_token(token)?;
    let id = Uuid::parse_str(&claims.sub)
        .map_err(|err| DomainError::validation_error(format!("Invalid token subject: {}", err)))?;

    Ok(CurrentUser {
        id,
        username: claims.username,
        email: claims.email,
        role: claims.role,
    })
}

async fn authenticate_basic(
    state: &AppState,
    username: &str,
    password: &str,
) -> Result<CurrentUser, AuthMiddlewareError> {
    if let Some(auth_svc) = state.auth_service.as_ref()
        && let Err(secs) = auth_svc.login_lockout.check(username)
    {
        tracing::warn!(
            username = %username,
            lockout_remaining_secs = secs,
            "Account locked — too many failed attempts"
        );
        return Err(AuthMiddlewareError::Unauthorized);
    }

    let app_password_service = if let Some(service) = state.app_password_service.as_ref() {
        service
    } else if let Some(nextcloud) = state.nextcloud.as_ref() {
        &nextcloud.app_passwords
    } else {
        return Err(AuthMiddlewareError::ServiceUnavailable);
    };

    let (id, username, email, role) = app_password_service
        .verify_basic_auth(username, password)
        .await
        .map_err(|_| AuthMiddlewareError::Unauthorized)?;

    Ok(CurrentUser {
        id,
        username,
        email,
        role,
    })
}

async fn record_dav_auth_failure(state: &AppState, failure: CreateDavAuthFailureDto) {
    if let Some(repo) = state.dav_auth_failure_repository.as_ref()
        && let Err(err) = repo.record_failure(failure).await
    {
        tracing::warn!("Failed to record DAV authentication failure: {}", err);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthMiddlewareError {
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Unauthorized")]
    BasicUnauthorized,
    #[error("Authentication service unavailable")]
    ServiceUnavailable,
}

impl IntoResponse for AuthMiddlewareError {
    fn into_response(self) -> Response {
        match self {
            AuthMiddlewareError::Unauthorized => {
                AppError::unauthorized("Authentication required").into_response()
            }
            AuthMiddlewareError::BasicUnauthorized => (
                StatusCode::UNAUTHORIZED,
                [(header::WWW_AUTHENTICATE, "Basic realm=\"OxiCloud\"")],
                "Unauthorized",
            )
                .into_response(),
            AuthMiddlewareError::ServiceUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Authentication service unavailable",
            )
                .into_response(),
        }
    }
}
