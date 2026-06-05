#![allow(clippy::collapsible_if)]
/**
 * CalDAV Handler Module
 *
 * This module implements the CalDAV protocol (RFC 4791) endpoints for OxiCloud.
 * It provides calendar access and management through standard CalDAV methods,
 * allowing clients like Thunderbird, Apple Calendar, and GNOME Calendar to sync.
 *
 * Supported methods:
 * - OPTIONS: Advertise CalDAV capabilities
 * - PROPFIND: List calendars and their properties
 * - REPORT: Query events (calendar-query, calendar-multiget)
 * - MKCALENDAR: Create a new calendar
 * - PUT: Create/update calendar events (.ics)
 * - GET: Retrieve calendar event data
 * - DELETE: Remove calendars or events
 * - PROPPATCH: Modify calendar properties
 */
use axum::{
    Router,
    body::{self, Body},
    http::{HeaderMap, HeaderName, Request, StatusCode, header},
    response::{Redirect, Response},
};
use bytes::Buf;
use percent_encoding::percent_decode_str;
use std::fmt::Write;
use std::sync::Arc;

use crate::application::adapters::caldav_adapter::{CalDavAdapter, CalDavReportType};
use crate::application::adapters::webdav_adapter::{PropFindRequest, PropFindType};
use crate::application::dtos::calendar_dto::{
    CalendarObjectDeleteConditionDto, CalendarObjectDeleteStatusDto, CalendarObjectPutConditionDto,
    CalendarObjectPutStatusDto, CreateCalendarDto, DeleteCalendarObjectDto, PutCalendarObjectDto,
    UpdateCalendarDto,
};
use crate::application::ports::calendar_ports::CalendarUseCase;
use crate::application::ports::dav_principal_ports::DavPrincipalDiscoveryUseCase;
use crate::application::services::calendar_service::CalendarService;
use crate::common::di::AppState;
use crate::interfaces::errors::AppError;
use crate::interfaces::middleware::auth::{AuthUser, CurrentUser};

const HEADER_DAV: HeaderName = HeaderName::from_static("dav");

/// Maximum allowed request body size for CalDAV XML/iCal endpoints (1 MB).
/// Prevents OOM/DoS via unbounded body buffering.
const MAX_CALDAV_BODY: usize = 1_048_576;

/// Creates CalDAV routes with full path prefixes.
///
/// Uses `merge()` instead of `nest()` to avoid Axum's trailing-slash routing gap.
/// Registers `/caldav`, `/caldav/`, and `/caldav/{*path}` explicitly.
pub fn caldav_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/caldav/{*path}", axum::routing::any(handle_caldav_methods))
        .route("/caldav/", axum::routing::any(handle_caldav_methods_root))
        .route("/caldav", axum::routing::any(handle_caldav_methods_root))
        .route(
            "/dav/calendars/{*path}",
            axum::routing::any(handle_dav_calendar_methods),
        )
}

/// Creates RFC 6764 well-known discovery routes.
/// These are public (no auth) and simply redirect to the CalDAV root.
pub fn well_known_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/.well-known/caldav",
            axum::routing::get(handle_well_known_caldav),
        )
        .route(
            "/.well-known/carddav",
            axum::routing::get(handle_well_known_carddav),
        )
}

pub async fn handle_well_known_caldav() -> Redirect {
    Redirect::permanent("/caldav/")
}

async fn handle_well_known_carddav() -> Redirect {
    Redirect::permanent("/carddav/")
}

async fn handle_caldav_methods_root(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    req: Request<Body>,
) -> Result<Response<Body>, AppError> {
    handle_caldav_methods_inner(state, req, String::new()).await
}

async fn handle_caldav_methods(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    req: Request<Body>,
) -> Result<Response<Body>, AppError> {
    let uri = req.uri().clone();
    let path = extract_caldav_path(uri.path());
    reject_path_traversal(&path)?;
    handle_caldav_methods_inner(state, req, path).await
}

async fn handle_dav_calendar_methods(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    req: Request<Body>,
) -> Result<Response<Body>, AppError> {
    let uri = req.uri().clone();
    let prefix = "/dav/calendars/";
    let encoded = uri.path().strip_prefix(prefix).unwrap_or_default();
    let path = percent_decode_str(encoded.trim_end_matches('/'))
        .decode_utf8_lossy()
        .into_owned();

    reject_path_traversal(&path)?;
    handle_caldav_methods_inner(state, req, format!("calendars/{}", path)).await
}

async fn handle_caldav_methods_inner(
    state: Arc<AppState>,
    req: Request<Body>,
    path: String,
) -> Result<Response<Body>, AppError> {
    let method = req.method().clone();

    match method.as_str() {
        "OPTIONS" => handle_options().await,
        "PROPFIND" => handle_propfind(state, req, &path).await,
        "REPORT" => handle_report(state, req, &path).await,
        "MKCALENDAR" => handle_mkcalendar(state, req, &path).await,
        "PUT" => handle_put(state, req, &path).await,
        "GET" => handle_get(state, req, &path).await,
        "DELETE" => handle_delete(state, req, &path).await,
        "PROPPATCH" => handle_proppatch(state, req, &path).await,
        _ => Err(AppError::method_not_allowed(format!(
            "Method not allowed: {}",
            method
        ))),
    }
}

/// Extract the CalDAV path from the full URI path, percent-decoding the result.
fn extract_caldav_path(uri_path: &str) -> String {
    let encoded = if let Some(pos) = uri_path.find("/caldav/") {
        let after = &uri_path[pos + 8..];
        after.trim_end_matches('/')
    } else if uri_path.ends_with("/caldav") {
        ""
    } else {
        uri_path.trim_start_matches('/').trim_end_matches('/')
    };
    percent_decode_str(encoded).decode_utf8_lossy().into_owned()
}

/// Reject paths that contain path-traversal segments (`.` or `..`).
fn reject_path_traversal(path: &str) -> Result<(), AppError> {
    for segment in path.split('/') {
        if segment == ".." || segment == "." {
            return Err(AppError::bad_request(
                "Path must not contain '.' or '..' segments",
            ));
        }
    }
    Ok(())
}

// ─── Helper: strip optional username prefix from CalDAV path ─────────
//
// The `calendar-home-set` discovery property returns `/caldav/{username}/`,
// so standard clients (DAVx5, Apple Calendar, Thunderbird) will prefix all
// subsequent requests with the username segment.  The handlers below expect
// paths of the form `{calendar_id}` or `{calendar_id}/{event}.ics`, so we
// need to detect and strip the leading username when present.
//
// Heuristic: if the first path segment is a valid UUID it is already a
// calendar ID; otherwise treat it as a username and skip it.

fn strip_username_prefix(path: &str) -> &str {
    if let Some(pos) = path.find('/') {
        let first = &path[..pos];
        if uuid::Uuid::parse_str(first).is_ok() {
            // First segment is a UUID → no username prefix
            path
        } else {
            // First segment is not a UUID → treat as username, return the rest
            &path[pos + 1..]
        }
    } else {
        // Single segment (no slash)
        if uuid::Uuid::parse_str(path).is_ok() {
            path
        } else {
            // Single non-UUID segment (bare username) → nothing useful after it
            ""
        }
    }
}

// ─── Helper: extract user from request ───────────────────────────────

fn parse_dav_calendar_collection_path(path: &str) -> Option<(String, String)> {
    let trimmed = path.trim_matches('/');
    let segments: Vec<&str> = trimmed
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    match segments.as_slice() {
        ["calendars", username, calendar_key]
            if !calendar_key.to_ascii_lowercase().ends_with(".ics") =>
        {
            Some(((*username).to_string(), (*calendar_key).to_string()))
        }
        _ => None,
    }
}

fn parse_dav_calendar_object_path(path: &str) -> Option<(String, String, String)> {
    let trimmed = path.trim_matches('/');
    let segments: Vec<&str> = trimmed
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    match segments.as_slice() {
        ["calendars", username, calendar_key, resource_name] => Some((
            (*username).to_string(),
            (*calendar_key).to_string(),
            (*resource_name).to_string(),
        )),
        _ => None,
    }
}

async fn find_calendar_id_by_owner_username_and_slug(
    state: &AppState,
    owner_username: &str,
    calendar_slug: &str,
) -> Result<Option<String>, AppError> {
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        AppError::new(
            StatusCode::NOT_IMPLEMENTED,
            "Database pool is not configured",
            "NotImplemented",
        )
    })?;

    let calendar_id = sqlx::query_scalar::<_, uuid::Uuid>(
        r#"
        SELECT c.id
        FROM caldav.calendars c
        INNER JOIN auth.users u ON u.id = c.owner_id
        WHERE u.username = $1
          AND c.slug = $2
        "#,
    )
    .bind(owner_username)
    .bind(calendar_slug)
    .fetch_optional(&**pool)
    .await
    .map_err(|e| AppError::internal_error(format!("Failed to resolve calendar path: {e}")))?;

    Ok(calendar_id.map(|id| id.to_string()))
}

async fn resolve_calendar_id_for_request_path(
    state: &AppState,
    calendar_service: &CalendarService,
    user: &AuthUser,
    owner_username: Option<&str>,
    calendar_key: &str,
) -> Result<String, AppError> {
    let calendar_id = if uuid::Uuid::parse_str(calendar_key).is_ok() {
        calendar_key.to_string()
    } else if let Some(owner_username) = owner_username {
        find_calendar_id_by_owner_username_and_slug(state, owner_username, calendar_key)
            .await?
            .ok_or_else(|| AppError::not_found("Calendar not found"))?
    } else {
        calendar_service
            .find_calendar_by_slug_for_owner(calendar_key, user.id)
            .await
            .map_err(AppError::from)?
            .id
    };

    let access = calendar_service
        .get_calendar_access(&calendar_id, user.id)
        .await
        .map_err(AppError::from)?;

    if !access.can_read {
        return Err(AppError::not_found("Calendar not found"));
    }

    Ok(calendar_id)
}

fn dav_need_privileges_response(path: &str, privilege: &str) -> Response<Body> {
    let href = if path.starts_with("/caldav/") || path.starts_with("/dav/") {
        path.to_string()
    } else if path.starts_with("calendars/") {
        format!("/dav/{}", path.trim_matches('/'))
    } else {
        format!("/caldav/{}", path.trim_matches('/'))
    };

    xml_error_response(
        StatusCode::FORBIDDEN,
        &format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<D:error xmlns:D="DAV:">
  <D:need-privileges>
    <D:resource>
      <D:href>{}</D:href>
      <D:privilege>
        <D:{}/>
      </D:privilege>
    </D:resource>
  </D:need-privileges>
</D:error>"#,
            escape_xml_text(&href),
            privilege
        ),
    )
}

async fn ensure_write_access_for_calendar_id(
    calendar_service: &CalendarService,
    user: &AuthUser,
    calendar_id: &str,
    request_path: &str,
    privilege: &str,
) -> Result<Option<Response<Body>>, AppError> {
    match calendar_service
        .ensure_calendar_write_access(calendar_id, user.id)
        .await
    {
        Ok(()) => Ok(None),
        Err(err) if err.kind == crate::domain::errors::ErrorKind::AccessDenied => {
            Ok(Some(dav_need_privileges_response(request_path, privilege)))
        }
        Err(err) => Err(AppError::from(err)),
    }
}

fn extract_user(req: &Request<Body>) -> Result<AuthUser, AppError> {
    req.extensions()
        .get::<Arc<CurrentUser>>()
        .cloned()
        .map(AuthUser)
        .ok_or_else(|| AppError::unauthorized("Authentication required"))
}

fn get_calendar_service(state: &AppState) -> Result<&Arc<CalendarService>, AppError> {
    state.calendar_use_case.as_ref().ok_or_else(|| {
        AppError::new(
            StatusCode::NOT_IMPLEMENTED,
            "CalDAV service is not configured",
            "NotImplemented",
        )
    })
}

fn get_dav_principal_service(
    state: &AppState,
) -> Result<&Arc<crate::application::services::dav_principal_service::DavPrincipalService>, AppError>
{
    state.dav_principal_service.as_ref().ok_or_else(|| {
        AppError::new(
            StatusCode::NOT_IMPLEMENTED,
            "DAV principal discovery service is not configured",
            "NotImplemented",
        )
    })
}

fn reject_xml_entities(body: &[u8]) -> Result<(), AppError> {
    let body = std::str::from_utf8(body)
        .map_err(|_| AppError::bad_request("XML request body must be valid UTF-8"))?;
    let upper = body.to_ascii_uppercase();
    if upper.contains("<!DOCTYPE") || upper.contains("<!ENTITY") {
        return Err(AppError::bad_request(
            "DOCTYPE and ENTITY declarations are not allowed in PROPFIND XML",
        ));
    }
    Ok(())
}

// ─── OPTIONS ─────────────────────────────────────────────────────────

async fn handle_options() -> Result<Response<Body>, AppError> {
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(HEADER_DAV, "1, 3, calendar-access, addressbook")
        .header(
            header::ALLOW,
            "OPTIONS, GET, PUT, DELETE, PROPFIND, PROPPATCH, REPORT, MKCALENDAR",
        )
        .body(Body::empty())
        .unwrap())
}

// ─── PROPFIND ────────────────────────────────────────────────────────

async fn handle_propfind(
    state: Arc<AppState>,
    req: Request<Body>,
    path: &str,
) -> Result<Response<Body>, AppError> {
    let depth = req
        .headers()
        .get("Depth")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("0")
        .to_string();

    let user = extract_user(&req)?;
    let calendar_service = get_calendar_service(&state)?;
    let dav_principal_service = get_dav_principal_service(&state)?;

    let body_bytes = body::to_bytes(req.into_body(), MAX_CALDAV_BODY)
        .await
        .map_err(|e| AppError::bad_request(format!("Failed to read request body: {}", e)))?;

    if !body_bytes.is_empty() {
        reject_xml_entities(&body_bytes)?;
    }

    let propfind_request = if body_bytes.is_empty() {
        PropFindRequest {
            prop_find_type: PropFindType::AllProp,
        }
    } else {
        crate::application::adapters::webdav_adapter::WebDavAdapter::parse_propfind(
            body_bytes.reader(),
        )
        .map_err(|e| AppError::bad_request(format!("Failed to parse PROPFIND: {}", e)))?
    };

    if path.is_empty() {
        let calendars = if depth == "0" {
            vec![]
        } else {
            calendar_service
                .list_my_calendars(user.id)
                .await
                .map_err(|e| AppError::internal_error(format!("Failed to list calendars: {}", e)))?
        };

        let base_href = "/caldav/";
        let home_sets = dav_principal_service
            .get_principal_home_sets(user.id)
            .await
            .map_err(|e| {
                AppError::internal_error(format!("Failed to fetch DAV principal: {}", e))
            })?;
        let mut response_body = Vec::new();
        CalDavAdapter::generate_root_propfind_response_with_home_sets(
            &mut response_body,
            &calendars,
            &propfind_request,
            base_href,
            &home_sets,
        )
        .map_err(|e| AppError::internal_error(format!("Failed to generate XML: {}", e)))?;

        return Ok(Response::builder()
            .status(StatusCode::MULTI_STATUS)
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .header(HEADER_DAV, "1, calendar-access, access-control")
            .body(Body::from(response_body))
            .unwrap());
    }

    if path.starts_with("principals/") || path == "principals" {
        let username = path.strip_prefix("principals/").unwrap_or(&user.username);
        let username = if username.is_empty() {
            &user.username
        } else {
            username
        };

        let principal_path = format!("/caldav/principals/{}/", username.trim_end_matches('/'));
        let home_sets = match dav_principal_service
            .get_principal_home_sets_by_path(&principal_path, user.id)
            .await
        {
            Ok(home_sets) => home_sets,
            Err(_) => dav_principal_service
                .get_principal_home_sets(user.id)
                .await
                .map_err(|e| {
                    AppError::internal_error(format!("Failed to fetch DAV principal: {}", e))
                })?,
        };
        let mut response_body = Vec::new();
        CalDavAdapter::generate_principal_propfind_response_with_home_sets(
            &mut response_body,
            &propfind_request,
            &home_sets,
        )
        .map_err(|e| AppError::internal_error(format!("Failed to generate XML: {}", e)))?;

        return Ok(Response::builder()
            .status(StatusCode::MULTI_STATUS)
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .header(HEADER_DAV, "1, calendar-access, access-control")
            .body(Body::from(response_body))
            .unwrap());
    }

    if let Some((owner_username, calendar_key)) = parse_dav_calendar_collection_path(path) {
        let calendar_id = resolve_calendar_id_for_request_path(
            &state,
            calendar_service,
            &user,
            Some(&owner_username),
            &calendar_key,
        )
        .await?;

        let calendar = calendar_service
            .get_calendar(&calendar_id, user.id)
            .await
            .map_err(AppError::from)?;

        let events = if depth != "0" {
            calendar_service
                .list_events(&calendar_id, None, None, user.id)
                .await
                .unwrap_or_default()
        } else {
            vec![]
        };

        let base_href = format!("/dav/calendars/{}/{}/", owner_username, calendar_key);
        let mut response_body = Vec::new();
        CalDavAdapter::generate_calendar_collection_propfind(
            &mut response_body,
            &calendar,
            &events,
            &propfind_request,
            &base_href,
            &depth,
        )
        .map_err(|e| AppError::internal_error(format!("Failed to generate XML: {}", e)))?;

        return Ok(Response::builder()
            .status(StatusCode::MULTI_STATUS)
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .header(HEADER_DAV, "1, calendar-access, access-control")
            .body(Body::from(response_body))
            .unwrap());
    }

    let parts: Vec<&str> = path.splitn(2, '/').collect();
    let first_segment = parts[0];
    let first_is_uuid = uuid::Uuid::parse_str(first_segment).is_ok();

    if parts.len() == 1 {
        let calendar_result = if first_is_uuid {
            calendar_service.get_calendar(first_segment, user.id).await
        } else {
            Err(crate::domain::errors::DomainError::new(
                crate::domain::errors::ErrorKind::NotFound,
                "Calendar",
                "Not a UUID",
            ))
        };

        if let Ok(calendar) = calendar_result {
            let events = if depth != "0" {
                calendar_service
                    .list_events(first_segment, None, None, user.id)
                    .await
                    .unwrap_or_default()
            } else {
                vec![]
            };

            let base_href = &format!("/caldav/{}/", first_segment);
            let mut response_body = Vec::new();
            CalDavAdapter::generate_calendar_collection_propfind(
                &mut response_body,
                &calendar,
                &events,
                &propfind_request,
                base_href,
                &depth,
            )
            .map_err(|e| AppError::internal_error(format!("Failed to generate XML: {}", e)))?;

            Ok(Response::builder()
                .status(StatusCode::MULTI_STATUS)
                .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
                .header(HEADER_DAV, "1, calendar-access, access-control")
                .body(Body::from(response_body))
                .unwrap())
        } else {
            let calendars = calendar_service
                .list_my_calendars(user.id)
                .await
                .map_err(|e| {
                    AppError::internal_error(format!("Failed to list calendars: {}", e))
                })?;

            let base_href = &format!("/caldav/{}/", first_segment);
            let mut response_body = Vec::new();
            CalDavAdapter::generate_calendars_propfind_response(
                &mut response_body,
                &calendars,
                &propfind_request,
                base_href,
            )
            .map_err(|e| AppError::internal_error(format!("Failed to generate XML: {}", e)))?;

            Ok(Response::builder()
                .status(StatusCode::MULTI_STATUS)
                .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
                .header(HEADER_DAV, "1, calendar-access, access-control")
                .body(Body::from(response_body))
                .unwrap())
        }
    } else {
        let rest = parts[1];
        let (calendar_id, event_path) = if first_is_uuid {
            (first_segment.to_string(), rest.to_string())
        } else {
            let sub_parts: Vec<&str> = rest.splitn(2, '/').collect();
            if sub_parts.len() == 1 {
                let calendar_id = resolve_calendar_id_for_request_path(
                    &state,
                    calendar_service,
                    &user,
                    None,
                    sub_parts[0],
                )
                .await?;

                let cal = calendar_service
                    .get_calendar(&calendar_id, user.id)
                    .await
                    .map_err(AppError::from)?;

                let events = if depth != "0" {
                    calendar_service
                        .list_events(&calendar_id, None, None, user.id)
                        .await
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                let base_href = &format!("/caldav/{}/{}/", first_segment, sub_parts[0]);
                let mut response_body = Vec::new();
                CalDavAdapter::generate_calendar_collection_propfind(
                    &mut response_body,
                    &cal,
                    &events,
                    &propfind_request,
                    base_href,
                    &depth,
                )
                .map_err(|e| AppError::internal_error(format!("Failed to generate XML: {}", e)))?;

                return Ok(Response::builder()
                    .status(StatusCode::MULTI_STATUS)
                    .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
                    .header(HEADER_DAV, "1, calendar-access, access-control")
                    .body(Body::from(response_body))
                    .unwrap());
            }

            (sub_parts[0].to_string(), sub_parts[1].to_string())
        };

        validate_calendar_resource_name(&event_path)?;

        let event = calendar_service
            .get_event_by_resource_name(&calendar_id, &event_path, user.id)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found(format!("Event not found: {}", event_path)))?;

        let base_href = &format!("/caldav/{}/", calendar_id);
        let report_type = CalDavReportType::CalendarMultiget {
            hrefs: vec![format!("{}{}", base_href, event.resource_name)],
            props: vec![],
        };

        let mut response_body = Vec::new();
        CalDavAdapter::generate_calendar_events_response(
            &mut response_body,
            std::slice::from_ref(&event),
            &report_type,
            base_href,
        )
        .map_err(|e| AppError::internal_error(format!("Failed to generate XML: {}", e)))?;

        Ok(Response::builder()
            .status(StatusCode::MULTI_STATUS)
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .header(HEADER_DAV, "1, calendar-access, access-control")
            .body(Body::from(response_body))
            .unwrap())
    }
}

// ─── REPORT ──────────────────────────────────────────────────────────

async fn handle_report(
    state: Arc<AppState>,
    req: Request<Body>,
    path: &str,
) -> Result<Response<Body>, AppError> {
    let user = extract_user(&req)?;
    let calendar_service = get_calendar_service(&state)?;

    let body_bytes = body::to_bytes(req.into_body(), MAX_CALDAV_BODY)
        .await
        .map_err(|e| AppError::bad_request(format!("Failed to read request body: {}", e)))?;

    let report = CalDavAdapter::parse_report(body_bytes.reader())
        .map_err(|e| AppError::bad_request(format!("Failed to parse REPORT: {}", e)))?;

    let effective_path = strip_username_prefix(path);
    let calendar_id = effective_path.split('/').next().unwrap_or(effective_path);

    if calendar_id.is_empty() {
        return Err(AppError::bad_request("Calendar ID required in path"));
    }

    let events = match &report {
        CalDavReportType::CalendarQuery { time_range, .. } => {
            if let Some((start, end)) = time_range {
                calendar_service
                    .get_events_in_range(calendar_id, *start, *end, user.id)
                    .await
                    .map_err(|e| {
                        AppError::internal_error(format!("Failed to query events: {}", e))
                    })?
            } else {
                calendar_service
                    .list_events(calendar_id, None, None, user.id)
                    .await
                    .map_err(|e| {
                        AppError::internal_error(format!("Failed to list events: {}", e))
                    })?
            }
        }
        CalDavReportType::CalendarMultiget { hrefs, .. } => {
            let all_events = calendar_service
                .list_events(calendar_id, None, None, user.id)
                .await
                .map_err(|e| AppError::internal_error(format!("Failed to list events: {}", e)))?;

            all_events
                .into_iter()
                .filter(|evt| {
                    hrefs.iter().any(|href| {
                        href.trim_end_matches('/')
                            .rsplit('/')
                            .next()
                            .map(|last| last == evt.resource_name)
                            .unwrap_or(false)
                    })
                })
                .collect()
        }
        CalDavReportType::SyncCollection { .. } => calendar_service
            .list_events(calendar_id, None, None, user.id)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to list events: {}", e)))?,
    };

    let base_href = &format!("/caldav/{}/", calendar_id);
    let mut response_body = Vec::new();
    CalDavAdapter::generate_calendar_events_response(
        &mut response_body,
        &events,
        &report,
        base_href,
    )
    .map_err(|e| AppError::internal_error(format!("Failed to generate XML: {}", e)))?;

    Ok(Response::builder()
        .status(StatusCode::MULTI_STATUS)
        .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
        .body(Body::from(response_body))
        .unwrap())
}

// ─── MKCALENDAR ──────────────────────────────────────────────────────

async fn handle_mkcalendar(
    state: Arc<AppState>,
    req: Request<Body>,
    path: &str,
) -> Result<Response<Body>, AppError> {
    let user = extract_user(&req)?;
    let calendar_service = get_calendar_service(&state)?;

    if let Some((owner_username, calendar_key)) = parse_dav_calendar_collection_path(path) {
        if let Some(existing_calendar_id) =
            find_calendar_id_by_owner_username_and_slug(&state, &owner_username, &calendar_key)
                .await?
        {
            let access = calendar_service
                .get_calendar_access(&existing_calendar_id, user.id)
                .await
                .map_err(AppError::from)?;

            if !access.can_read {
                return Err(AppError::not_found("Calendar not found"));
            }

            if !access.can_write {
                return Ok(dav_need_privileges_response(path, "write"));
            }

            return Err(AppError::conflict("Calendar already exists"));
        }

        if owner_username != user.username {
            return Ok(dav_need_privileges_response(path, "bind"));
        }
    }

    let body_bytes = body::to_bytes(req.into_body(), MAX_CALDAV_BODY)
        .await
        .map_err(|e| AppError::bad_request(format!("Failed to read request body: {}", e)))?;

    let effective_path = path.trim_matches('/');
    let slug_segment = effective_path
        .split('/')
        .next_back()
        .filter(|segment| !segment.is_empty())
        .ok_or_else(|| AppError::bad_request("Calendar slug required in path"))?;

    let slug = percent_decode_str(slug_segment)
        .decode_utf8()
        .map_err(|e| AppError::bad_request(format!("Invalid calendar slug encoding: {}", e)))?
        .to_string();

    let (name, description, color) = if body_bytes.is_empty() {
        (slug.clone(), None, None)
    } else {
        CalDavAdapter::parse_mkcalendar(body_bytes.reader())
            .map_err(|e| AppError::bad_request(format!("Failed to parse MKCALENDAR: {}", e)))?
    };

    let create_dto = CreateCalendarDto {
        slug: Some(slug.clone()),
        name,
        description,
        color,
        timezone_text: None,
        supported_components: None,
        calendar_order: None,
        is_public: Some(false),
    };

    calendar_service
        .create_calendar(create_dto, user.id)
        .await
        .map_err(AppError::from)?;

    Ok(Response::builder()
        .status(StatusCode::CREATED)
        .body(Body::empty())
        .unwrap())
}

// ─── PUT (.ics) ──────────────────────────────────────────────────────

async fn handle_put(
    state: Arc<AppState>,
    req: Request<Body>,
    path: &str,
) -> Result<Response<Body>, AppError> {
    let user = extract_user(&req)?;
    let calendar_service = get_calendar_service(&state)?;

    let (owner_username, calendar_key, resource_name) = parse_calendar_object_path(path)?;
    let calendar_id = resolve_calendar_id_for_request_path(
        &state,
        calendar_service,
        &user,
        owner_username.as_deref(),
        &calendar_key,
    )
    .await?;

    if let Some(response) =
        ensure_write_access_for_calendar_id(calendar_service, &user, &calendar_id, path, "write")
            .await?
    {
        return Ok(response);
    }

    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();

    if !is_text_calendar_content_type(content_type) {
        return Ok(xml_error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            r#"<?xml version="1.0" encoding="utf-8"?>
<D:error xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <C:supported-calendar-data/>
  <D:responsedescription>This calendar collection only accepts text/calendar resources.</D:responsedescription>
</D:error>"#,
        ));
    }

    let condition = match put_condition_from_headers(req.headers()) {
        Ok(condition) => condition,
        Err(err) if err.status_code == StatusCode::PRECONDITION_FAILED => {
            return Ok(xml_error_response(
                StatusCode::PRECONDITION_FAILED,
                &format!(
                    r#"<?xml version="1.0" encoding="utf-8"?>
<D:error xmlns:D="DAV:">
  <D:responsedescription>{}</D:responsedescription>
</D:error>"#,
                    escape_xml_text(&err.message)
                ),
            ));
        }
        Err(err) => return Err(err),
    };

    let body_bytes = body::to_bytes(req.into_body(), MAX_CALDAV_BODY)
        .await
        .map_err(|e| AppError::bad_request(format!("Failed to read request body: {}", e)))?;

    if body_bytes.is_empty() || body_bytes.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(xml_error_response(
            StatusCode::BAD_REQUEST,
            r#"<?xml version="1.0" encoding="utf-8"?>
<D:error xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <C:valid-calendar-data/>
  <D:responsedescription>Calendar object resource body must not be empty.</D:responsedescription>
</D:error>"#,
        ));
    }

    let ical_data = String::from_utf8(body_bytes.to_vec())
        .map_err(|e| AppError::bad_request(format!("Invalid UTF-8 in iCalendar data: {}", e)))?;

    let put = PutCalendarObjectDto {
        calendar_id,
        resource_name,
        ical_data,
        condition,
    };

    let result = match calendar_service.put_calendar_object(put, user.id).await {
        Ok(result) => result,
        Err(err) if err.entity_type == "CalDavPreconditionFailed" => {
            return Ok(xml_error_response(
                StatusCode::PRECONDITION_FAILED,
                &format!(
                    r#"<?xml version="1.0" encoding="utf-8"?>
<D:error xmlns:D="DAV:">
  <D:responsedescription>{}</D:responsedescription>
</D:error>"#,
                    escape_xml_text(&err.message)
                ),
            ));
        }
        Err(err) if err.entity_type == "CalDavUidConflict" => {
            return Ok(xml_error_response(
                StatusCode::FORBIDDEN,
                &format!(
                    r#"<?xml version="1.0" encoding="utf-8"?>
<D:error xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <C:no-uid-conflict/>
  <D:responsedescription>{}</D:responsedescription>
</D:error>"#,
                    escape_xml_text(&err.message)
                ),
            ));
        }
        Err(err) if err.kind == crate::domain::errors::ErrorKind::InvalidInput => {
            return Ok(xml_error_response(
                StatusCode::FORBIDDEN,
                &format!(
                    r#"<?xml version="1.0" encoding="utf-8"?>
<D:error xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <C:valid-calendar-data/>
  <D:responsedescription>{}</D:responsedescription>
</D:error>"#,
                    escape_xml_text(&err.message)
                ),
            ));
        }
        Err(err) => return Err(AppError::from(err)),
    };

    let status = match result.status {
        CalendarObjectPutStatusDto::Created => StatusCode::CREATED,
        CalendarObjectPutStatusDto::Updated => StatusCode::NO_CONTENT,
    };

    Ok(Response::builder()
        .status(status)
        .header(header::ETAG, format!("\"{}\"", result.event.etag))
        .body(Body::empty())
        .unwrap())
}

fn is_text_calendar_content_type(content_type: &str) -> bool {
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    media_type == "text/calendar"
}

fn put_condition_from_headers(
    headers: &HeaderMap,
) -> Result<CalendarObjectPutConditionDto, AppError> {
    if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH) {
        let value = if_none_match
            .to_str()
            .map_err(|_| AppError::bad_request("Invalid If-None-Match header"))?
            .trim();

        if value == "*" {
            return Ok(CalendarObjectPutConditionDto::IfNoneMatchAny);
        }

        return Err(AppError::bad_request(
            "Only If-None-Match: * is supported for calendar object PUT",
        ));
    }

    if let Some(if_match) = headers.get(header::IF_MATCH) {
        let value = if_match
            .to_str()
            .map_err(|_| AppError::bad_request("Invalid If-Match header"))?
            .trim();

        if value.starts_with("W/") {
            return Err(AppError::precondition_failed(
                "Weak ETags are not valid for CalDAV If-Match",
            ));
        }

        let etag = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .ok_or_else(|| AppError::bad_request("If-Match ETag must be quoted"))?;

        if etag.is_empty() {
            return Err(AppError::bad_request("If-Match ETag must not be empty"));
        }

        return Ok(CalendarObjectPutConditionDto::IfMatch(etag.to_string()));
    }

    Ok(CalendarObjectPutConditionDto::None)
}

fn parse_calendar_object_path(path: &str) -> Result<(Option<String>, String, String), AppError> {
    if let Some((owner_username, calendar_key, resource_name)) =
        parse_dav_calendar_object_path(path)
    {
        validate_calendar_resource_name(&resource_name)?;
        return Ok((Some(owner_username), calendar_key, resource_name));
    }

    let trimmed = path.trim_matches('/');
    let effective_path = strip_username_prefix(trimmed);
    let parts: Vec<&str> = effective_path.splitn(2, '/').collect();

    if parts.len() != 2 {
        return Err(AppError::bad_request(
            "Path must identify a calendar object resource",
        ));
    }

    let calendar_key = parts[0].to_string();
    let resource_name = parts[1].to_string();
    validate_calendar_resource_name(&resource_name)?;

    Ok((None, calendar_key, resource_name))
}

fn validate_calendar_resource_name(resource_name: &str) -> Result<(), AppError> {
    if resource_name.trim().is_empty()
        || resource_name.contains('/')
        || resource_name.contains('\\')
        || resource_name == "."
        || resource_name == ".."
        || !resource_name.to_ascii_lowercase().ends_with(".ics")
    {
        return Err(AppError::bad_request(
            "Calendar object resource name must be a .ics file name",
        ));
    }

    Ok(())
}

fn xml_error_response(status: StatusCode, body: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ─── GET (.ics) ──────────────────────────────────────────────────────

async fn handle_get(
    state: Arc<AppState>,
    req: Request<Body>,
    path: &str,
) -> Result<Response<Body>, AppError> {
    let user = extract_user(&req)?;
    let calendar_service = get_calendar_service(&state)?;

    let effective_path = strip_username_prefix(path);
    let parts: Vec<&str> = effective_path.splitn(2, '/').collect();
    let calendar_id = parts[0];

    if parts.len() < 2 {
        // GET on calendar collection
        let events = calendar_service
            .list_events(calendar_id, None, None, user.id)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to list events: {}", e)))?;

        let calendar = calendar_service
            .get_calendar(calendar_id, user.id)
            .await
            .map_err(|e| AppError::not_found(format!("Calendar not found: {}", e)))?;

        let ical = generate_full_calendar_ical(&calendar.name, &events);

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
            .header(header::ETAG, format!("\"{}\"", calendar.id))
            .body(Body::from(ical))
            .unwrap())
    } else {
        let event_file = parts[1];
        validate_calendar_resource_name(event_file)?;

        let event = calendar_service
            .get_event_by_resource_name(calendar_id, event_file, user.id)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to get event: {}", e)))?
            .ok_or_else(|| AppError::not_found(format!("Event not found: {}", event_file)))?;

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
            .header(header::ETAG, format!("\"{}\"", event.etag))
            .body(Body::from(event.ical_data))
            .unwrap())
    }
}

fn generate_full_calendar_ical(
    calendar_name: &str,
    events: &[crate::application::dtos::calendar_dto::CalendarEventDto],
) -> String {
    // Pre-estimate: ~200 bytes header + ~320 bytes per event
    let mut buf = String::with_capacity(256 + events.len() * 320);
    let _ = write!(
        buf,
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//OxiCloud//NONSGML Calendar//EN\r\nX-WR-CALNAME:{}\r\n",
        calendar_name
    );
    for event in events {
        write_vevent(&mut buf, event);
    }
    buf.push_str("END:VCALENDAR\r\n");
    buf
}

/// Writes a VEVENT block directly into `buf` — zero intermediate allocations.
fn write_vevent(
    buf: &mut String,
    event: &crate::application::dtos::calendar_dto::CalendarEventDto,
) {
    let _ = write!(
        buf,
        "BEGIN:VEVENT\r\nUID:{}\r\nSUMMARY:{}\r\nDTSTART:{}\r\nDTEND:{}\r\n",
        event.ical_uid,
        event.summary.replace('\n', "\\n"),
        event.start_time.format("%Y%m%dT%H%M%SZ"),
        event.end_time.format("%Y%m%dT%H%M%SZ"),
    );
    if let Some(ref desc) = event.description {
        let _ = write!(buf, "DESCRIPTION:{}\r\n", desc.replace('\n', "\\n"));
    }
    if let Some(ref loc) = event.location {
        let _ = write!(buf, "LOCATION:{}\r\n", loc);
    }
    if let Some(ref rrule) = event.rrule {
        let _ = write!(buf, "RRULE:{}\r\n", rrule);
    }
    let _ = write!(
        buf,
        "DTSTAMP:{}\r\nCREATED:{}\r\nLAST-MODIFIED:{}\r\nEND:VEVENT\r\n",
        event.updated_at.format("%Y%m%dT%H%M%SZ"),
        event.created_at.format("%Y%m%dT%H%M%SZ"),
        event.updated_at.format("%Y%m%dT%H%M%SZ"),
    );
}

// ─── DELETE ──────────────────────────────────────────────────────────

async fn handle_delete(
    state: Arc<AppState>,
    req: Request<Body>,
    path: &str,
) -> Result<Response<Body>, AppError> {
    let user = extract_user(&req)?;

    if let Some(collection_path) = parse_calendar_collection_path(path)? {
        return handle_delete_calendar_collection(state, req, user, collection_path).await;
    }

    handle_delete_calendar_object(state, req, user, path).await
}

async fn handle_delete_calendar_object(
    state: Arc<AppState>,
    req: Request<Body>,
    user: AuthUser,
    path: &str,
) -> Result<Response<Body>, AppError> {
    let calendar_service = get_calendar_service(&state)?;
    let condition = delete_condition_from_headers(req.headers())?;

    let (owner_username, calendar_key, resource_name) = parse_calendar_object_path(path)?;
    let calendar_id = resolve_calendar_id_for_request_path(
        &state,
        calendar_service,
        &user,
        owner_username.as_deref(),
        &calendar_key,
    )
    .await?;

    if let Some(response) =
        ensure_write_access_for_calendar_id(calendar_service, &user, &calendar_id, path, "write")
            .await?
    {
        return Ok(response);
    }

    let result = calendar_service
        .delete_calendar_object(
            DeleteCalendarObjectDto {
                calendar_id,
                resource_name,
                condition,
            },
            user.id,
        )
        .await
        .map_err(AppError::from)?;

    let status = match result.status {
        CalendarObjectDeleteStatusDto::Deleted => StatusCode::NO_CONTENT,
        CalendarObjectDeleteStatusDto::NotFound => StatusCode::NOT_FOUND,
        CalendarObjectDeleteStatusDto::PreconditionFailed => StatusCode::PRECONDITION_FAILED,
    };

    Ok(Response::builder()
        .status(status)
        .body(Body::empty())
        .unwrap())
}

async fn handle_delete_calendar_collection(
    state: Arc<AppState>,
    req: Request<Body>,
    user: AuthUser,
    collection_path: CalendarCollectionPath,
) -> Result<Response<Body>, AppError> {
    if !is_valid_delete_collection_depth(req.headers())? {
        return Ok(xml_error_response(
            StatusCode::BAD_REQUEST,
            r#"<?xml version="1.0" encoding="utf-8"?>
<D:error xmlns:D="DAV:">
  <D:valid-depth/>
</D:error>"#,
        ));
    }

    let calendar_service = get_calendar_service(&state)?;
    let calendar_id_string = resolve_calendar_id_for_request_path(
        &state,
        calendar_service,
        &user,
        collection_path.username.as_deref(),
        &collection_path.calendar_key,
    )
    .await?;

    if let Some(response) = ensure_write_access_for_calendar_id(
        calendar_service,
        &user,
        &calendar_id_string,
        &format!(
            "calendars/{}/{}",
            collection_path
                .username
                .as_deref()
                .unwrap_or(&user.username),
            collection_path.calendar_key
        ),
        "write",
    )
    .await?
    {
        return Ok(response);
    }

    let condition = delete_condition_from_headers(req.headers())?;
    let pool = state.db_pool.as_ref().ok_or_else(|| {
        AppError::new(
            StatusCode::NOT_IMPLEMENTED,
            "Database pool is not configured",
            "NotImplemented",
        )
    })?;

    let calendar_id = uuid::Uuid::parse_str(&calendar_id_string)
        .map_err(|_| AppError::internal_error("Resolved calendar ID is invalid"))?;

    if !collection_if_match_satisfied(&condition, &calendar_id) {
        return Ok(xml_error_response(
            StatusCode::PRECONDITION_FAILED,
            r#"<?xml version="1.0" encoding="utf-8"?>
<D:error xmlns:D="DAV:"/>"#,
        ));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to begin calendar delete: {e}")))?;

    sqlx::query(
        r#"
        DELETE FROM caldav.calendar_events
        WHERE calendar_id = $1
        "#,
    )
    .bind(calendar_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal_error(format!("Failed to delete calendar events: {e}")))?;

    sqlx::query(
        r#"
        DELETE FROM caldav.calendar_properties
        WHERE calendar_id = $1
        "#,
    )
    .bind(calendar_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal_error(format!("Failed to delete calendar properties: {e}")))?;

    sqlx::query(
        r#"
        DELETE FROM caldav.calendar_shares
        WHERE calendar_id = $1
        "#,
    )
    .bind(calendar_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal_error(format!("Failed to delete calendar shares: {e}")))?;

    let result = sqlx::query(
        r#"
        DELETE FROM caldav.calendars
        WHERE id = $1
          AND owner_id = $2
        "#,
    )
    .bind(calendar_id)
    .bind(user.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::internal_error(format!("Failed to delete calendar: {e}")))?;

    if result.rows_affected() != 1 {
        tx.rollback().await.map_err(|e| {
            AppError::internal_error(format!("Failed to roll back calendar delete: {e}"))
        })?;
        return Ok(xml_error_response(
            StatusCode::NOT_FOUND,
            r#"<?xml version="1.0" encoding="utf-8"?>
<D:error xmlns:D="DAV:"/>"#,
        ));
    }

    tx.commit()
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to commit calendar delete: {e}")))?;

    Ok(Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .unwrap())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CalendarCollectionPath {
    username: Option<String>,
    calendar_key: String,
}

fn parse_calendar_collection_path(path: &str) -> Result<Option<CalendarCollectionPath>, AppError> {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return Ok(None);
    }

    let segments: Vec<&str> = trimmed
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    let collection_path = match segments.as_slice() {
        ["calendars", username, calendar_key] => Some(CalendarCollectionPath {
            username: Some((*username).to_string()),
            calendar_key: (*calendar_key).to_string(),
        }),
        [username, calendar_key] if !calendar_key.to_ascii_lowercase().ends_with(".ics") => {
            Some(CalendarCollectionPath {
                username: Some((*username).to_string()),
                calendar_key: (*calendar_key).to_string(),
            })
        }
        [calendar_key] if uuid::Uuid::parse_str(calendar_key).is_ok() => {
            Some(CalendarCollectionPath {
                username: None,
                calendar_key: (*calendar_key).to_string(),
            })
        }
        _ => None,
    };

    if let Some(collection_path) = &collection_path {
        validate_calendar_collection_segment(&collection_path.calendar_key)?;
        if let Some(username) = &collection_path.username {
            validate_calendar_collection_segment(username)?;
        }
    }

    Ok(collection_path)
}

fn validate_calendar_collection_segment(segment: &str) -> Result<(), AppError> {
    if segment.trim().is_empty()
        || segment.contains('/')
        || segment.contains('\\')
        || segment == "."
        || segment == ".."
    {
        return Err(AppError::bad_request(
            "Calendar collection path segment is invalid",
        ));
    }

    Ok(())
}

fn is_valid_delete_collection_depth(headers: &HeaderMap) -> Result<bool, AppError> {
    let Some(depth) = headers.get("Depth") else {
        return Ok(true);
    };

    let depth = depth
        .to_str()
        .map_err(|_| AppError::bad_request("Invalid Depth header"))?
        .trim();

    Ok(depth.eq_ignore_ascii_case("infinity"))
}

fn collection_if_match_satisfied(
    condition: &CalendarObjectDeleteConditionDto,
    calendar_id: &uuid::Uuid,
) -> bool {
    match condition {
        CalendarObjectDeleteConditionDto::None => true,
        CalendarObjectDeleteConditionDto::IfMatchAny => true,
        CalendarObjectDeleteConditionDto::IfMatch(etags) => {
            let collection_etag = calendar_id.to_string();
            etags.iter().any(|etag| etag == &collection_etag)
        }
    }
}

fn delete_condition_from_headers(
    headers: &HeaderMap,
) -> Result<CalendarObjectDeleteConditionDto, AppError> {
    let Some(if_match) = headers.get(header::IF_MATCH) else {
        return Ok(CalendarObjectDeleteConditionDto::None);
    };

    let value = if_match
        .to_str()
        .map_err(|_| AppError::bad_request("Invalid If-Match header"))?
        .trim();

    if value == "*" {
        return Ok(CalendarObjectDeleteConditionDto::IfMatchAny);
    }

    let mut etags = Vec::new();
    for raw in value.split(',') {
        let candidate = raw.trim();
        if candidate.starts_with("W/") {
            return Err(AppError::precondition_failed(
                "Weak ETags are not valid for CalDAV If-Match",
            ));
        }

        let etag = candidate
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .ok_or_else(|| AppError::bad_request("If-Match ETags must be quoted or *"))?;

        if etag.is_empty() {
            return Err(AppError::bad_request("If-Match ETag must not be empty"));
        }

        etags.push(etag.to_string());
    }

    if etags.is_empty() {
        return Err(AppError::bad_request("If-Match header must not be empty"));
    }

    Ok(CalendarObjectDeleteConditionDto::IfMatch(etags))
}

// ─── PROPPATCH ───────────────────────────────────────────────────────

async fn handle_proppatch(
    state: Arc<AppState>,
    req: Request<Body>,
    path: &str,
) -> Result<Response<Body>, AppError> {
    let user = extract_user(&req)?;
    let calendar_service = get_calendar_service(&state)?;

    let collection_path = parse_calendar_collection_path(path)?
        .ok_or_else(|| AppError::bad_request("Invalid calendar collection path"))?;

    let calendar_id_string = resolve_calendar_id_for_request_path(
        &state,
        calendar_service,
        &user,
        collection_path.username.as_deref(),
        &collection_path.calendar_key,
    )
    .await?;

    if let Some(response) = ensure_write_access_for_calendar_id(
        calendar_service,
        &user,
        &calendar_id_string,
        path,
        "PROPPATCH",
    )
    .await?
    {
        return Ok(response);
    }

    let body_bytes = body::to_bytes(req.into_body(), MAX_CALDAV_BODY)
        .await
        .map_err(|e| AppError::bad_request(format!("Failed to read request body: {}", e)))?;

    let (props_to_set, props_to_remove) =
        crate::application::adapters::webdav_adapter::WebDavAdapter::parse_proppatch(
            body_bytes.reader(),
        )
        .map_err(|e| AppError::bad_request(format!("Failed to parse PROPPATCH: {}", e)))?;

    let calendar_id = calendar_id_string.as_str();

    let mut update = UpdateCalendarDto {
        slug: None,
        name: None,
        description: None,
        color: None,
        timezone_text: None,
        supported_components: None,
        calendar_order: None,
        is_public: None,
    };

    for prop in &props_to_set {
        match prop.name.name.as_str() {
            "displayname" => update.name = Some(prop.value.clone().unwrap_or_default()),
            "calendar-description" => update.description = prop.value.clone(),
            "calendar-color" => update.color = prop.value.clone(),
            _ => {}
        }
    }

    if update.name.is_some() || update.description.is_some() || update.color.is_some() {
        calendar_service
            .update_calendar(calendar_id, update, user.id)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to update calendar: {}", e)))?;
    }

    let mut results = Vec::new();
    for prop in &props_to_set {
        results.push((&prop.name, true));
    }
    for prop in &props_to_remove {
        results.push((prop, true));
    }

    let href = format!("/caldav/{}", path);
    let mut response_body = Vec::new();
    crate::application::adapters::webdav_adapter::WebDavAdapter::generate_proppatch_response(
        &mut response_body,
        &href,
        &results,
    )
    .map_err(|e| AppError::internal_error(format!("Failed to generate XML: {}", e)))?;

    Ok(Response::builder()
        .status(StatusCode::MULTI_STATUS)
        .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
        .body(Body::from(response_body))
        .unwrap())
}

#[cfg(test)]
mod tests {
    use super::strip_username_prefix;

    #[test]
    fn test_strip_username_prefix_uuid_only() {
        let uuid = "ae8ae236-709f-4939-b766-37ad589ac7f2";
        assert_eq!(strip_username_prefix(uuid), uuid);
    }

    #[test]
    fn test_strip_username_prefix_uuid_with_event() {
        let path = "ae8ae236-709f-4939-b766-37ad589ac7f2/event.ics";
        assert_eq!(strip_username_prefix(path), path);
    }

    #[test]
    fn test_strip_username_prefix_username_and_uuid() {
        let path = "timm/ae8ae236-709f-4939-b766-37ad589ac7f2";
        assert_eq!(
            strip_username_prefix(path),
            "ae8ae236-709f-4939-b766-37ad589ac7f2"
        );
    }

    #[test]
    fn test_strip_username_prefix_username_uuid_and_event() {
        let path = "timm/ae8ae236-709f-4939-b766-37ad589ac7f2/event.ics";
        assert_eq!(
            strip_username_prefix(path),
            "ae8ae236-709f-4939-b766-37ad589ac7f2/event.ics"
        );
    }

    #[test]
    fn test_strip_username_prefix_bare_username() {
        assert_eq!(strip_username_prefix("timm"), "");
    }

    #[test]
    fn test_strip_username_prefix_empty() {
        assert_eq!(strip_username_prefix(""), "");
    }

    #[test]
    fn test_strip_username_prefix_email_style_username() {
        let path = "user@example.com/ae8ae236-709f-4939-b766-37ad589ac7f2/event.ics";
        assert_eq!(
            strip_username_prefix(path),
            "ae8ae236-709f-4939-b766-37ad589ac7f2/event.ics"
        );
    }
}
