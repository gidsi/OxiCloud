#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

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
    async fn caldav_options_advertises_mkcalendar_support() {
        let response = super::caldav_handler::handle_options()
            .await
            .expect("OPTIONS handler should build a response");

        assert_eq!(response.status(), StatusCode::OK);

        let allow_header = response
            .headers()
            .get("allow")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();

        // Must explicitly support and advertise creating calendars to external clients
        assert!(allow_header.contains("MKCALENDAR"), "Server should advertise MKCALENDAR method support");
    }
}
