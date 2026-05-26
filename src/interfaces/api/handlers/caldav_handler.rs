use crate::domain::errors::ErrorKind;
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
    http::{HeaderName, Request, StatusCode, header},
    response::{Redirect, Response},
};
use bytes::Buf;
use percent_encoding::percent_decode_str;
use std::sync::Arc;

use crate::application::adapters::caldav_adapter::{CalDavAdapter, CalDavReportType};
use crate::application::adapters::webdav_adapter::{PropFindRequest, PropFindType};
use crate::application::dtos::calendar_dto::{
    CalendarDto, CreateCalendarDto, EventPutPreconditionDto, PutEventICalDto, UpdateCalendarDto,
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

async fn handle_well_known_caldav() -> Redirect {
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

// ─── Helpers: CalDAV path resolution ────────────────────────────────

fn path_segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn ensure_calendar_home_user(username: &str, user: &AuthUser) -> Result<(), AppError> {
    if username != user.username {
        return Err(AppError::forbidden(
            "Cannot access another user's calendar home",
        ));
    }
    Ok(())
}

struct ResolvedCalendarTarget {
    calendar: CalendarDto,
    event_path: Option<String>,
    base_href: String,
}

async fn resolve_calendar_target(
    calendar_service: &CalendarService,
    path: &str,
    user: &AuthUser,
) -> Result<ResolvedCalendarTarget, AppError> {
    let segments = path_segments(path);
    let Some(first_segment) = segments.first().copied() else {
        return Err(AppError::bad_request("Calendar path required"));
    };

    if uuid::Uuid::parse_str(first_segment).is_ok() {
        let calendar = calendar_service
            .get_calendar(first_segment, user.id)
            .await
            .map_err(|e| AppError::not_found(format!("Calendar not found: {}", e)))?;
        let event_path = if segments.len() > 1 {
            Some(segments[1..].join("/"))
        } else {
            None
        };

        return Ok(ResolvedCalendarTarget {
            calendar,
            event_path,
            base_href: format!("/caldav/{}/", first_segment),
        });
    }

    ensure_calendar_home_user(first_segment, user)?;

    let Some(calendar_path) = segments.get(1).copied() else {
        return Err(AppError::bad_request("Calendar path required"));
    };

    let calendar = calendar_service
        .get_calendar_by_path(calendar_path, user.id)
        .await
        .map_err(|e| AppError::not_found(format!("Calendar not found: {}", e)))?;
    let event_path = if segments.len() > 2 {
        Some(segments[2..].join("/"))
    } else {
        None
    };

    Ok(ResolvedCalendarTarget {
        calendar,
        event_path,
        base_href: format!("/caldav/{}/{}/", first_segment, calendar_path),
    })
}

// ─── Helper: extract user from request ───────────────────────────────

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
            "DOCTYPE and ENTITY declarations are not allowed in CalDAV XML",
        ));
    }
    Ok(())
}

fn caldav_xml_body_allowed(
    content_type: Option<&axum::http::HeaderValue>,
    body_is_empty: bool,
) -> Result<(), AppError> {
    if body_is_empty {
        return Ok(());
    }

    let Some(content_type) = content_type else {
        return Ok(());
    };

    let content_type = content_type
        .to_str()
        .map_err(|_| AppError::bad_request("Content-Type header must be valid ASCII"))?
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    match content_type.as_str() {
        "text/xml" | "application/xml" => Ok(()),
        _ => Err(AppError::unsupported_media_type(
            "MKCALENDAR request body must be text/xml or application/xml",
        )),
    }
}

fn extract_mkcalendar_path(path: &str, user: &AuthUser) -> Result<String, AppError> {
    let segments = path_segments(path);

    let [username, calendar_path] = segments.as_slice() else {
        return Err(AppError::conflict(
            "MKCALENDAR target must be a direct child of the calendar home",
        ));
    };

    ensure_calendar_home_user(username, user)?;

    if calendar_path.is_empty()
        || uuid::Uuid::parse_str(calendar_path).is_ok()
        || *calendar_path == "."
        || *calendar_path == ".."
    {
        return Err(AppError::conflict(
            "MKCALENDAR target must be a direct child of the calendar home",
        ));
    }

    reject_path_traversal(calendar_path)?;
    Ok((*calendar_path).to_string())
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

    // Parse PROPFIND request
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
        // Root CalDAV path — return discovery properties + list user's calendars
        // At depth 0, only return root entry; at depth 1+, also include calendars
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

        Ok(Response::builder()
            .status(StatusCode::MULTI_STATUS)
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .body(Body::from(response_body))
            .unwrap())
    } else if path.starts_with("principals/") || path == "principals" {
        // Principal resource — return user principal properties
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

        Ok(Response::builder()
            .status(StatusCode::MULTI_STATUS)
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .body(Body::from(response_body))
            .unwrap())
    } else {
        let segments = path_segments(path);
        let first_segment = segments[0];

        if segments.len() == 1 && uuid::Uuid::parse_str(first_segment).is_err() {
            ensure_calendar_home_user(first_segment, &user)?;

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

            return Ok(Response::builder()
                .status(StatusCode::MULTI_STATUS)
                .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
                .body(Body::from(response_body))
                .unwrap());
        }

        let target = resolve_calendar_target(calendar_service, path, &user).await?;
        let calendar_id = target.calendar.id.as_str();

        if let Some(event_path) = target.event_path.as_deref() {
            let ical_uid = event_path.trim_end_matches(".ics");

            let events = calendar_service
                .list_events(calendar_id, None, None, user.id)
                .await
                .map_err(|e| AppError::internal_error(format!("Failed to list events: {}", e)))?;

            let event = events
                .iter()
                .find(|e| e.ical_uid == ical_uid)
                .ok_or_else(|| AppError::not_found(format!("Event not found: {}", ical_uid)))?;

            let report_type = CalDavReportType::CalendarMultiget {
                hrefs: vec![format!("{}{}.ics", target.base_href, ical_uid)],
                props: vec![],
            };

            let mut response_body = Vec::new();
            CalDavAdapter::generate_calendar_events_response(
                &mut response_body,
                std::slice::from_ref(event),
                &report_type,
                &target.base_href,
            )
            .map_err(|e| AppError::internal_error(format!("Failed to generate XML: {}", e)))?;

            Ok(Response::builder()
                .status(StatusCode::MULTI_STATUS)
                .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
                .body(Body::from(response_body))
                .unwrap())
        } else {
            let events = if depth != "0" {
                calendar_service
                    .list_events(calendar_id, None, None, user.id)
                    .await
                    .unwrap_or_default()
            } else {
                vec![]
            };

            let mut response_body = Vec::new();

            CalDavAdapter::generate_calendar_collection_propfind(
                &mut response_body,
                &target.calendar,
                &events,
                &propfind_request,
                &target.base_href,
                &depth,
            )
            .map_err(|e| AppError::internal_error(format!("Failed to generate XML: {}", e)))?;

            Ok(Response::builder()
                .status(StatusCode::MULTI_STATUS)
                .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
                .body(Body::from(response_body))
                .unwrap())
        }
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
    let target = resolve_calendar_target(calendar_service, path, &user).await?;
    let calendar_id = target.calendar.id.as_str();

    let body_bytes = body::to_bytes(req.into_body(), MAX_CALDAV_BODY)
        .await
        .map_err(|e| AppError::bad_request(format!("Failed to read request body: {}", e)))?;

    let report = CalDavAdapter::parse_report(body_bytes.reader())
        .map_err(|e| AppError::bad_request(format!("Failed to parse REPORT: {}", e)))?;

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
                .filter(|evt| hrefs.iter().any(|href| href.contains(&evt.ical_uid)))
                .collect()
        }
        CalDavReportType::SyncCollection { .. } => calendar_service
            .list_events(calendar_id, None, None, user.id)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to list events: {}", e)))?,
    };

    let mut response_body = Vec::new();
    CalDavAdapter::generate_calendar_events_response(
        &mut response_body,
        &events,
        &report,
        &target.base_href,
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
    let calendar_path = extract_mkcalendar_path(path, &user)?;
    let location = format!("/caldav/{}/{}/", user.username, calendar_path);

    let content_type = req.headers().get(header::CONTENT_TYPE).cloned();
    let body_bytes = body::to_bytes(req.into_body(), MAX_CALDAV_BODY)
        .await
        .map_err(|e| AppError::bad_request(format!("Failed to read request body: {}", e)))?;

    caldav_xml_body_allowed(content_type.as_ref(), body_bytes.is_empty())?;
    reject_xml_entities(&body_bytes)?;

    let (name, description, color) = if body_bytes.is_empty() {
        (calendar_path.clone(), None, None)
    } else {
        let (displayname, description, color) =
            CalDavAdapter::parse_mkcalendar(body_bytes.reader())
                .map_err(|e| AppError::bad_request(format!("Failed to parse MKCALENDAR: {}", e)))?;
        let name = if displayname.trim().is_empty() {
            calendar_path.clone()
        } else {
            displayname
        };
        (name, description, color)
    };

    let create_dto = CreateCalendarDto {
        name,
        path: calendar_path,
        description,
        color,
        is_public: Some(false),
    };

    calendar_service
        .create_calendar(create_dto, user.id)
        .await
        .map_err(|e| match e.kind {
            ErrorKind::AlreadyExists => AppError::conflict(e.message),
            ErrorKind::InvalidInput => AppError::bad_request(e.message),
            _ => AppError::from(e),
        })?;

    Ok(Response::builder()
        .status(StatusCode::CREATED)
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::LOCATION, location)
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
    let target = resolve_calendar_target(calendar_service, path, &user).await?;
    let calendar_id = target.calendar.id.as_str();
    let resource_path = target.event_path.clone().ok_or_else(|| {
        AppError::bad_request(
            "Path must be {calendar_id}/{uid}.ics or {username}/{calendar_path}/{uid}.ics",
        )
    })?;

    caldav_calendar_body_allowed(req.headers().get(header::CONTENT_TYPE))?;
    let precondition = parse_put_precondition(&req)?;

    let body_bytes = body::to_bytes(req.into_body(), MAX_CALDAV_BODY)
        .await
        .map_err(|e| AppError::bad_request(format!("Failed to read request body: {}", e)))?;

    let ical_data = String::from_utf8(body_bytes.to_vec())
        .map_err(|e| AppError::bad_request(format!("Invalid UTF-8 in iCalendar data: {}", e)))?;

    validate_calendar_object_for_put(&ical_data)?;

    let put_dto = PutEventICalDto {
        calendar_id: calendar_id.to_string(),
        resource_path,
        ical_data,
        precondition,
    };

    let result = calendar_service
        .put_event_from_ical(put_dto, user.id)
        .await
        .map_err(caldav_put_error)?;

    Ok(Response::builder()
        .status(if result.created { StatusCode::CREATED } else { StatusCode::NO_CONTENT })
        .header(header::ETAG, quoted_etag(&result.event.etag))
        .body(Body::empty())
        .unwrap())
}


// ─── GET (.ics) ──────────────────────────────────────────────────────

async fn handle_get(
    state: Arc<AppState>,
    req: Request<Body>,
    path: &str,
) -> Result<Response<Body>, AppError> {
    let user = extract_user(&req)?;
    let calendar_service = get_calendar_service(&state)?;
    let target = resolve_calendar_target(calendar_service, path, &user).await?;
    let calendar_id = target.calendar.id.as_str();

    if let Some(event_file) = target.event_path.as_deref() {
        let event = calendar_service
            .get_event_by_resource_path(calendar_id, event_file, user.id)
            .await
            .map_err(|e| match e.kind {
                ErrorKind::NotFound => AppError::not_found(e.message),
                ErrorKind::AccessDenied => AppError::forbidden(e.message),
                _ => AppError::from(e),
            })?;

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
            .header(header::ETAG, quoted_etag(&event.etag))
            .body(Body::from(event.ical_data))
            .unwrap())
    } else {
        let events = calendar_service
            .list_events(calendar_id, None, None, user.id)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to list events: {}", e)))?;

        let ical = generate_full_calendar_ical(&target.calendar.name, &events);

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
            .header(header::ETAG, format!("\"{}\"", target.calendar.ctag))
            .body(Body::from(ical))
            .unwrap())
    }
}

fn generate_full_calendar_ical(
    calendar_name: &str,
    events: &[crate::application::dtos::calendar_dto::CalendarEventDto],
) -> String {
    let mut buf = String::with_capacity(256 + events.iter().map(|e| e.ical_data.len()).sum::<usize>());
    buf.push_str("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//OxiCloud//NONSGML Calendar//EN\r\n");
    buf.push_str(&format!("X-WR-CALNAME:{}\r\n", calendar_name));
    for event in events {
        append_vevent_from_stored_ical(&mut buf, &event.ical_data);
    }
    buf.push_str("END:VCALENDAR\r\n");
    buf
}

fn append_vevent_from_stored_ical(buf: &mut String, ical_data: &str) {
    let normalized = ical_data.replace("\r\n", "\n");
    let mut in_vevent = false;
    for line in normalized.lines() {
        if line.trim().eq_ignore_ascii_case("BEGIN:VEVENT") {
            in_vevent = true;
        }
        if in_vevent {
            buf.push_str(line);
            buf.push_str("\r\n");
        }
        if line.trim().eq_ignore_ascii_case("END:VEVENT") {
            in_vevent = false;
        }
    }
}

fn quoted_etag(etag: &str) -> String {
    format!("\"{}\"", etag.trim_matches('"'))
}

fn parse_put_precondition(req: &Request<Body>) -> Result<EventPutPreconditionDto, AppError> {
    if let Some(value) = req.headers().get(header::IF_NONE_MATCH) {
        let value = value.to_str().map_err(|_| AppError::bad_request("Invalid If-None-Match header"))?;
        if value.trim() == "*" {
            return Ok(EventPutPreconditionDto::IfNoneMatchAny);
        }
    }
    if let Some(value) = req.headers().get(header::IF_MATCH) {
        let value = value.to_str().map_err(|_| AppError::bad_request("Invalid If-Match header"))?;
        let trimmed = value.trim();
        if trimmed == "*" {
            return Ok(EventPutPreconditionDto::IfMatchAny);
        }
        return Ok(EventPutPreconditionDto::IfMatch(trimmed.trim_matches('"').to_string()));
    }
    Ok(EventPutPreconditionDto::None)
}

fn caldav_calendar_body_allowed(content_type: Option<&axum::http::HeaderValue>) -> Result<(), AppError> {
    let Some(value) = content_type else {
        return Err(caldav_xml_error(StatusCode::UNSUPPORTED_MEDIA_TYPE, "supported-calendar-data"));
    };
    let value = value.to_str().map_err(|_| caldav_xml_error(StatusCode::UNSUPPORTED_MEDIA_TYPE, "supported-calendar-data"))?.to_ascii_lowercase();
    if value.split(';').next().map(str::trim) == Some("text/calendar") {
        Ok(())
    } else {
        Err(caldav_xml_error(StatusCode::UNSUPPORTED_MEDIA_TYPE, "supported-calendar-data"))
    }
}

fn validate_calendar_object_for_put(ical_data: &str) -> Result<(), AppError> {
    if !ical_data.contains("BEGIN:VCALENDAR") || !ical_data.contains("END:VCALENDAR") || !ical_data.contains("BEGIN:VEVENT") || !ical_data.contains("END:VEVENT") {
        return Err(caldav_xml_error(StatusCode::FORBIDDEN, "valid-calendar-object-resource"));
    }
    if ical_data.contains("BEGIN:VTODO") || ical_data.contains("BEGIN:VJOURNAL") {
        return Err(caldav_xml_error(StatusCode::FORBIDDEN, "supported-calendar-component"));
    }
    for required in ["UID", "DTSTAMP", "DTSTART", "DTEND", "SUMMARY"] {
        if extract_ical_property_for_put(ical_data, required).is_none() {
            return Err(caldav_xml_error(StatusCode::FORBIDDEN, "valid-calendar-object-resource"));
        }
    }
    Ok(())
}

fn extract_ical_property_for_put(ical_data: &str, property_name: &str) -> Option<String> {
    for line in ical_data.replace("\r\n", "\n").lines() {
        let (name, value) = line.trim().split_once(':')?;
        if name.split(';').next().unwrap_or(name).eq_ignore_ascii_case(property_name) && !value.trim().is_empty() {
            return Some(value.trim().to_string());
        }
    }
    None
}

fn caldav_xml_error(status: StatusCode, element: &str) -> AppError {
    AppError::new(
        status,
        format!("<?xml version=\"1.0\" encoding=\"utf-8\"?><D:error xmlns:D=\"DAV:\" xmlns:C=\"urn:ietf:params:xml:ns:caldav\"><C:{}/></D:error>", element),
        "CalDavError",
    )
}

fn caldav_put_error(e: crate::common::errors::DomainError) -> AppError {
    match e.kind {
        ErrorKind::InvalidInput if e.entity_type == "Precondition" => AppError::precondition_failed(e.message),
        ErrorKind::InvalidInput => caldav_xml_error(StatusCode::FORBIDDEN, "valid-calendar-object-resource"),
        ErrorKind::UnsupportedOperation => caldav_xml_error(StatusCode::FORBIDDEN, "supported-calendar-component"),
        ErrorKind::AccessDenied if e.entity_type == "CalendarEvent" => caldav_xml_error(StatusCode::FORBIDDEN, "no-uid-conflict"),
        ErrorKind::AccessDenied => AppError::forbidden(e.message),
        ErrorKind::NotFound => AppError::not_found(e.message),
        _ => AppError::from(e),
    }
}

// ─── DELETE ──────────────────────────────────────────────────────────

async fn handle_delete(
    state: Arc<AppState>,
    req: Request<Body>,
    path: &str,
) -> Result<Response<Body>, AppError> {
    let user = extract_user(&req)?;
    let calendar_service = get_calendar_service(&state)?;
    let target = resolve_calendar_target(calendar_service, path, &user).await?;
    let calendar_id = target.calendar.id.as_str();

    if let Some(event_file) = target.event_path.as_deref() {
        let ical_uid = event_file.trim_end_matches(".ics");

        let events = calendar_service
            .list_events(calendar_id, None, None, user.id)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to list events: {}", e)))?;

        let event = events
            .iter()
            .find(|e| e.ical_uid == ical_uid)
            .ok_or_else(|| AppError::not_found(format!("Event not found: {}", ical_uid)))?;

        calendar_service
            .delete_event(&event.id, user.id)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to delete event: {}", e)))?;
    } else {
        calendar_service
            .delete_calendar(calendar_id, user.id)
            .await
            .map_err(|e| AppError::internal_error(format!("Failed to delete calendar: {}", e)))?;
    }

    Ok(Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .unwrap())
}

// ─── PROPPATCH ───────────────────────────────────────────────────────

async fn handle_proppatch(
    state: Arc<AppState>,
    req: Request<Body>,
    path: &str,
) -> Result<Response<Body>, AppError> {
    let user = extract_user(&req)?;
    let calendar_service = get_calendar_service(&state)?;

    let body_bytes = body::to_bytes(req.into_body(), MAX_CALDAV_BODY)
        .await
        .map_err(|e| AppError::bad_request(format!("Failed to read request body: {}", e)))?;

    let (props_to_set, props_to_remove) =
        crate::application::adapters::webdav_adapter::WebDavAdapter::parse_proppatch(
            body_bytes.reader(),
        )
        .map_err(|e| AppError::bad_request(format!("Failed to parse PROPPATCH: {}", e)))?;

    let target = resolve_calendar_target(calendar_service, path, &user).await?;
    let calendar_id = target.calendar.id.as_str();

    let mut update = UpdateCalendarDto {
        name: None,
        path: None,
        description: None,
        color: None,
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

    let href = target.base_href.clone();
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
    use super::extract_mkcalendar_path;
    use crate::application::dtos::user_dto::CurrentUser;
    use crate::interfaces::middleware::auth::AuthUser;
    use axum::http::StatusCode;
    use std::sync::Arc;
    use uuid::Uuid;

    fn test_user() -> AuthUser {
        AuthUser(Arc::new(CurrentUser {
            id: Uuid::new_v4(),
            username: "timm".to_string(),
            email: "timm@example.com".to_string(),
            role: "user".to_string(),
        }))
    }

    #[test]
    fn extract_mkcalendar_path_accepts_direct_child_of_calendar_home() {
        let user = test_user();

        let calendar_path = extract_mkcalendar_path("timm/work-calendar", &user).unwrap();

        assert_eq!(calendar_path, "work-calendar");
    }

    #[test]
    fn extract_mkcalendar_path_rejects_single_calendar_segment() {
        let user = test_user();

        let err = extract_mkcalendar_path("work-calendar", &user).unwrap_err();

        assert_eq!(err.status_code, StatusCode::CONFLICT);
    }

    #[test]
    fn extract_mkcalendar_path_rejects_calendar_home_itself() {
        let user = test_user();

        let err = extract_mkcalendar_path("timm", &user).unwrap_err();

        assert_eq!(err.status_code, StatusCode::CONFLICT);
    }

    #[test]
    fn extract_mkcalendar_path_rejects_nested_targets() {
        let user = test_user();

        let err = extract_mkcalendar_path("timm/work-calendar/extra", &user).unwrap_err();

        assert_eq!(err.status_code, StatusCode::CONFLICT);
    }

    #[test]
    fn extract_mkcalendar_path_rejects_other_user_calendar_home() {
        let user = test_user();

        let err = extract_mkcalendar_path("other-user/work-calendar", &user).unwrap_err();

        assert_eq!(err.status_code, StatusCode::FORBIDDEN);
    }

    #[test]
    fn extract_mkcalendar_path_rejects_uuid_calendar_path() {
        let user = test_user();
        let path = format!("timm/{}", Uuid::new_v4());

        let err = extract_mkcalendar_path(&path, &user).unwrap_err();

        assert_eq!(err.status_code, StatusCode::CONFLICT);
    }

    #[test]
    fn extract_mkcalendar_path_rejects_dot_segment() {
        let user = test_user();

        let err = extract_mkcalendar_path("timm/.", &user).unwrap_err();

        assert_eq!(err.status_code, StatusCode::CONFLICT);
    }

    #[test]
    fn extract_mkcalendar_path_rejects_dot_dot_segment() {
        let user = test_user();

        let err = extract_mkcalendar_path("timm/..", &user).unwrap_err();

        assert_eq!(err.status_code, StatusCode::CONFLICT);
    }
}
