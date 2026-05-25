use axum::{
    extract::{FromRequestParts, Request, State},
    http::{HeaderMap, StatusCode, header, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::{
    headers::{authorization::Basic, Authorization},
    typed_header::TypedHeader,
};
use std::convert::Infallible;
use std::sync::Arc;
use uuid::Uuid;

use crate::common::di::AppState;

// Re-export CurrentUser from application layer for use in handlers
pub use crate::application::dtos::user_dto::CurrentUser;
use crate::application::dtos::dav_auth_failure_dto::CreateDavAuthFailureDto;
use crate::application::ports::auth_ports::TokenServicePort;
use crate::application::ports::dav_auth_failure_ports::DavAuthFailureStoragePort;
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

/// Reusable extractor that gets the user_id of the authenticated user.
/// Automatically extracted from the `CurrentUser` inserted by the auth middleware.
///
/// Usage in handlers:
/// ```ignore
/// async fn my_handler(CurrentUserId(user_id): CurrentUserId) -> impl IntoResponse { ... }
/// ```
#[derive(Clone, Debug)]
pub struct CurrentUserId(pub Uuid);

// Implement FromRequestParts for AuthUser — allows using `auth_user: AuthUser` in handlers.
// Cost: 1 atomic increment (~1 ns) instead of 3 String clones (~100 ns + 3 mallocs).
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Arc<CurrentUser>>()
            .cloned()
            .map(AuthUser)
            .ok_or(AuthError::UserNotFound)
    }
}

// Implement FromRequestParts for CurrentUserId — lightweight extractor for user_id only
impl<S> FromRequestParts<S> for CurrentUserId
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Arc<CurrentUser>>()
            .map(|cu| CurrentUserId(cu.id))
            .ok_or(AuthError::UserNotFound)
    }
}

/// Optional user ID extractor – never fails.
/// Yields `Some(id)` when auth middleware ran, `None` otherwise.
#[derive(Clone, Debug)]
pub struct OptionalUserId(pub Option<Uuid>);

impl<S> FromRequestParts<S> for OptionalUserId
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(OptionalUserId(
            parts.extensions.get::<Arc<CurrentUser>>().map(|cu| cu.id),
        ))
    }
}

// Error for authentication operations
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Token not provided")]
    TokenNotProvided,

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Token expired")]
    TokenExpired,

    #[error("User not found")]
    UserNotFound,

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Authentication service unavailable")]
    AuthServiceUnavailable,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::TokenNotProvided => {
                (StatusCode::UNAUTHORIZED, "Token not provided".to_string())
            }
            AuthError::InvalidToken(msg) => (StatusCode::UNAUTHORIZED, msg),
            AuthError::TokenExpired => (StatusCode::UNAUTHORIZED, "Token expired".to_string()),
            AuthError::UserNotFound => (StatusCode::UNAUTHORIZED, "User not found".to_string()),
            AuthError::AccessDenied(msg) => (StatusCode::FORBIDDEN, msg),
            AuthError::AuthServiceUnavailable => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Authentication service unavailable".to_string(),
            ),
        };

        let body = axum::Json(serde_json::json!({
            "error": error_message
        }));

        (status, body).into_response()
    }
}


fn is_dav_path(path: &str) -> bool {
    path.starts_with("/caldav") || path.starts_with("/carddav") || path.starts_with("/webdav")
}

fn dav_unauthorized_response() -> Response {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::WWW_AUTHENTICATE, r#"Basic realm="OxiCloud""#)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(axum::body::Body::from("Unauthorized"))
        .unwrap()
}

async fn record_dav_auth_failed(
    state: Arc<AppState>,
    client_ip: String,
    username: String,
    method: String,
    path: String,
    user_agent: String,
    reason: &'static str,
) {
    if !is_dav_path(&path) {
        return;
    }

    tracing::warn!(
        target: "audit",
        event = "AuthFailed",
        ip = %client_ip,
        username = %username,
        method = %method,
        path = %path,
        reason = %reason,
        "DAV authentication failed"
    );

    if let Some(repo) = state.dav_auth_failure_repository.as_ref() {
        let failure = CreateDavAuthFailureDto {
            client_ip,
            username,
            method,
            path,
            user_agent,
            reason: reason.to_string(),
            auth_scheme: "Basic".to_string(),
            protocol: "DAV".to_string(),
        };

        if let Err(err) = repo.record_failure(failure).await {
            tracing::warn!("Failed to persist DAV auth failure audit event: {}", err);
        }
    }
}

fn dav_auth_failure_context(
    request: &Request,
    headers: &HeaderMap,
    username: &str,
) -> Option<(String, String, String, String, String)> {
    if !is_dav_path(request.uri().path()) {
        return None;
    }

    let client_ip = extract_client_ip(request);
    let method = request.method().as_str().to_string();
    let path = request.uri().path().to_string();
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let username = username.to_string();

    Some((client_ip, username, method, path, user_agent))
}

async fn record_dav_auth_failed_for_request(
    state: Arc<AppState>,
    client_ip: String,
    username: String,
    method: String,
    path: String,
    user_agent: String,
    reason: &'static str,
) {
    record_dav_auth_failed(state, client_ip, username, method, path, user_agent, reason).await;
}

/// Secure authentication middleware.
///
/// Supports three authentication methods (tried in order):
/// 1. **Bearer JWT** — standard token in `Authorization: Bearer <token>`
/// 2. **Basic Auth with App Passwords** — for DAV clients (DAVx⁵, Thunderbird, rclone)
///    that send `Authorization: Basic base64(username:app_password)`
/// 3. **HttpOnly Cookie** — `oxicloud_access` cookie set by the login endpoint;
///    used by browser-based sessions so tokens are never exposed to JS.
///
/// Bearer is tried first; if no Bearer header is found, Basic is attempted,
/// then the cookie fallback.
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    let headers = request.headers().clone();
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    // ── 1. Try Bearer JWT ────────────────────────────────────────
    if let Some(header_value) = auth_header.as_deref() {
        if let Some(token_str) = header_value.strip_prefix("Bearer ") {
            let token_str = token_str.trim();
            if !token_str.is_empty() {
                tracing::debug!("Processing Bearer authentication token");

                if let Some(auth_service) = state.auth_service.as_ref() {
                    let token_service = &auth_service.token_service;
                    match token_service.validate_token(token_str) {
                        Ok(claims) => {
                            tracing::debug!(
                                "Token validated successfully for user: {}",
                                claims.username
                            );
                            let user_id = match Uuid::parse_str(&claims.sub) {
                                Ok(user_id) => user_id,
                                Err(_) => {
                                    return AuthError::InvalidToken(
                                        "Invalid user ID in token".to_string(),
                                    )
                                    .into_response();
                                }
                            };
                            let current_user = Arc::new(CurrentUser {
                                id: user_id,
                                username: claims.username,
                                email: claims.email,
                                role: claims.role,
                            });
                            request.extensions_mut().insert(current_user);
                            tracing::Span::current().record("user_id", user_id.to_string());
                            return next.run(request).await;
                        }
                        Err(e) => {
                            tracing::warn!("Bearer token validation failed: {}", e);
                            return AuthError::InvalidToken(format!("Invalid token: {}", e)).into_response();
                        }
                    }
                }
            }
        }

        // ── 2. Try Basic Auth with App Passwords ─────────────────
        if header_value.starts_with("Basic ") {
            tracing::debug!("Processing Basic authentication (app password)");
            let dav_request = is_dav_path(request.uri().path());

            let (mut parts, body) = request.into_parts();
            let typed_basic = TypedHeader::<Authorization<Basic>>::from_request_parts(&mut parts, &())
                .await
                .ok();
            request = Request::from_parts(parts, body);

            let (username, password) = match typed_basic {
                Some(TypedHeader(Authorization(credentials))) => (
                    credentials.username().to_string(),
                    credentials.password().to_string(),
                ),
                None => {
                    if let Some((client_ip, username, method, path, user_agent)) =
                        dav_auth_failure_context(&request, &headers, "")
                    {
                        record_dav_auth_failed_for_request(
                            state.clone(),
                            client_ip,
                            username,
                            method,
                            path,
                            user_agent,
                            "malformed_credentials",
                        )
                        .await;
                    }

                    if dav_request {
                        return dav_unauthorized_response();
                    }

                    return AuthError::InvalidToken(
                        "Invalid Basic auth format".to_string(),
                    )
                    .into_response();
                }
            };

            if let Some(app_pw_service) = state.app_password_service.as_ref() {
                match app_pw_service.verify_basic_auth(&username, &password).await {
                    Ok((user_id, uname, email, role)) => {
                        tracing::debug!(
                            "App password authentication successful for user: {}",
                            uname
                        );
                        let current_user = Arc::new(CurrentUser {
                            id: user_id,
                            username: uname,
                            email,
                            role,
                        });
                        request.extensions_mut().insert(current_user);
                        tracing::Span::current().record("user_id", user_id.to_string());
                        return next.run(request).await;
                    }
                    Err(e) => {
                        tracing::warn!("App password verification failed: {}", e);
                        if let Some((client_ip, audit_username, method, path, user_agent)) =
                            dav_auth_failure_context(&request, &headers, &username)
                        {
                            record_dav_auth_failed_for_request(
                                state.clone(),
                                client_ip,
                                audit_username,
                                method,
                                path,
                                user_agent,
                                "invalid_credentials",
                            )
                            .await;
                        }

                        if dav_request {
                            return dav_unauthorized_response();
                        }

                        return AuthError::InvalidToken(
                            "Invalid username or app password".to_string(),
                        ).into_response();
                    }
                }
            } else {
                tracing::warn!("Basic auth attempted but app password service not configured");
                if let Some((client_ip, audit_username, method, path, user_agent)) =
                    dav_auth_failure_context(&request, &headers, &username)
                {
                    record_dav_auth_failed_for_request(
                        state.clone(),
                        client_ip,
                        audit_username,
                        method,
                        path,
                        user_agent,
                        "app_passwords_disabled",
                    )
                    .await;
                }

                if dav_request {
                    return dav_unauthorized_response();
                }

                return AuthError::InvalidToken(
                    "App passwords are not enabled".to_string(),
                ).into_response();
            }
        }
    }

    // ── 3. Try HttpOnly cookie (browser sessions) ────────────────
    {
        use crate::interfaces::api::cookie_auth;

        if let Some(token_str) =
            cookie_auth::extract_cookie_value(&headers, cookie_auth::ACCESS_COOKIE)
            && !token_str.is_empty()
        {
            tracing::debug!("Processing cookie-based authentication");

            if let Some(auth_service) = state.auth_service.as_ref() {
                let token_service = &auth_service.token_service;
                match token_service.validate_token(&token_str) {
                    Ok(claims) => {
                        tracing::debug!("Cookie token validated for user: {}", claims.username);
                        let user_id = match Uuid::parse_str(&claims.sub) {
                            Ok(user_id) => user_id,
                            Err(_) => {
                                return AuthError::InvalidToken(
                                    "Invalid user ID in token".to_string(),
                                )
                                .into_response();
                            }
                        };
                        let current_user = Arc::new(CurrentUser {
                            id: user_id,
                            username: claims.username,
                            email: claims.email,
                            role: claims.role,
                        });
                        request.extensions_mut().insert(current_user);
                        request.extensions_mut().insert(CookieAuthenticated);
                        tracing::Span::current().record("user_id", user_id.to_string());
                        return next.run(request).await;
                    }
                    Err(e) => {
                        tracing::debug!("Cookie token validation failed: {}", e);
                        // Don't return error — fall through to "no token" so
                        // the browser gets a 401 and can redirect to /login.
                    }
                }
            }
        }
    }

    // No valid credentials found via any method.
    if state.auth_service.is_none() {
        tracing::error!("Auth middleware invoked but auth service is not configured");
        return AuthError::AuthServiceUnavailable.into_response();
    }

    // DAV protocol paths must receive a real Basic challenge and must never
    // fall through to browser-oriented JSON errors or the SPA login page.
    if is_dav_path(request.uri().path()) {
        if let Some((client_ip, username, method, path, user_agent)) =
            dav_auth_failure_context(&request, &headers, "")
        {
            record_dav_auth_failed_for_request(
                state.clone(),
                client_ip,
                username,
                method,
                path,
                user_agent,
                "missing_credentials",
            )
            .await;
        }
        return dav_unauthorized_response();
    }

    AuthError::TokenNotProvided.into_response()
}

/// Middleware to verify that the authenticated user has an admin role.
///
/// Must be applied AFTER auth_middleware, as it depends on
/// `CurrentUser` being present in the request extensions.
pub async fn require_admin(request: Request, next: Next) -> Response {
    // Get the CurrentUser inserted by auth_middleware
    if let Some(current_user) = request.extensions().get::<Arc<CurrentUser>>() {
        if current_user.role == "admin" {
            tracing::debug!("Admin access granted for user: {}", current_user.username);
            return next.run(request).await;
        }
        tracing::warn!(
            "Admin access denied for user: {} (role: {})",
            current_user.username,
            current_user.role
        );
    } else {
        tracing::warn!("Admin check failed: no authenticated user in request");
    }

    // Access denied
    let error = AuthError::AccessDenied("Admin role required".to_string());
    error.into_response()
}
