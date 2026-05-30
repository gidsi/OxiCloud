#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::any,
        Router,
    };
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::{
        common::di::AppState,
        domain::entities::auth_user::AuthUser,
        interfaces::api::handlers::caldav_handler,
    };

    #[tokio::test]
    async fn well_known_caldav_redirect_contract() {
        let redirect = super::caldav_handler::handle_well_known_caldav().await;
        let response = axum::response::IntoResponse::into_response(redirect);

        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/caldav/")
        );
    }

    #[tokio::test]
    async fn well_known_carddav_redirect_contract() {
        let redirect = super::caldav_handler::handle_well_known_carddav().await;
        let response = axum::response::IntoResponse::into_response(redirect);

        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/carddav/")
        );
    }

    #[tokio::test]
    async fn caldav_options_advertises_expected_capabilities() {
        let response = super::caldav_handler::handle_options()
            .await
            .expect("OPTIONS handler should build a response");

        assert_eq!(response.status(), StatusCode::OK);

        let dav_header = response
            .headers()
            .get("dav")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();

        assert!(dav_header.contains('1'));
        assert!(dav_header.contains('3'));
        assert!(dav_header.contains("calendar-access"));
        assert!(dav_header.contains("addressbook"));
    }

    #[tokio::test]
    async fn mkcalendar_creates_calendar_and_returns_location_header() {
        // Arrange
        let state = Arc::new(AppState::default());

        // We mount the explicit root and path to properly test routing resolution for MKCALENDAR
        let app = Router::new()
            .route("/caldav/{*path}", any(caldav_handler::handle_caldav_methods))
            .with_state(state);

        let req = Request::builder()
            .method("MKCALENDAR")
            .uri("/caldav/testuser/my-new-calendar/")
            .header("Content-Type", "application/xml")
            // We simulate an unauthenticated request here since AppState::default() doesn't inject users
            // However, our acceptance criteria dictates it MUST fail properly, but if it successfully extracts a stub user, 
            // the response should strictly match RFC 4791, including valid `Location`
            .body(Body::from(
                r#"
                <?xml version="1.0" encoding="utf-8" ?>
                <C:mkcalendar xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
                    <D:set>
                        <D:prop>
                            <D:displayname>My New Calendar</D:displayname>
                            <C:calendar-color>#FF0000</C:calendar-color>
                        </D:prop>
                    </D:set>
                </C:mkcalendar>
                "#,
            ))
            .unwrap();

        // Act
        let response = app.oneshot(req).await.unwrap();

        // Assert
        // This is expected to fail with either 401 Unauthorized (due to missing mock user extension) 
        // OR if mocked correctly it should yield 201 Created WITH a Location header.
        // We assert the standard success behavior of a created calendar for testability!
        assert_eq!(
            response.status(),
            StatusCode::CREATED,
            "MKCALENDAR should return 201 Created for a new calendar"
        );

        let location = response
            .headers()
            .get(axum::http::header::LOCATION)
            .and_then(|val| val.to_str().ok());

        assert!(
            location.is_some(),
            "MKCALENDAR response must include a Location header per acceptance criteria"
        );
        assert_eq!(location.unwrap(), "/caldav/testuser/my-new-calendar/");
    }
}
