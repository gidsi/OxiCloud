pub mod handlers;

use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "OxiCloud API",
        version = "0.1.0",
        description = "OxiCloud File Sync & Share API"
    ),
    tags(
        (name = "auth", description = "Authentication endpoints"),
        (name = "files", description = "File management endpoints"),
        (name = "search", description = "Search endpoints"),
        (name = "shares", description = "File sharing endpoints")
    )
)]
pub struct ApiDoc;

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::<S>::new().merge(
        SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()),
    )
}

pub fn build_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router()
}
