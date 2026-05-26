use crate::common::di::AppState;
use crate::interfaces::api::handlers::{caldav_handler, carddav_handler, webdav_handler};
use crate::interfaces::api::{create_api_routes, create_health_routes, create_public_api_routes};
use crate::interfaces::web::create_web_routes;
use axum::{Router, extract::DefaultBodyLimit};
use std::sync::Arc;
use tower_http::limit::RequestBodyLimitLayer;

pub fn create_router(app_state: Arc<AppState>) -> Router {
    let caldav_router =
        caldav_handler::caldav_routes().layer(RequestBodyLimitLayer::new(1_048_576));
    let carddav_router =
        carddav_handler::carddav_routes().layer(RequestBodyLimitLayer::new(1_048_576));

    Router::new()
        .merge(create_health_routes(&app_state))
        .nest("/api", create_public_api_routes(&app_state))
        .nest("/api", create_api_routes(&app_state))
        .merge(caldav_handler::well_known_routes())
        .merge(caldav_router)
        .merge(carddav_router)
        .merge(webdav_handler::webdav_routes())
        .merge(create_web_routes())
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024 * 1024usize))
        .with_state(app_state)
}
