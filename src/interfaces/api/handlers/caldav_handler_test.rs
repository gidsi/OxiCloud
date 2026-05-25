#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    #[tokio::test]
    async fn test_well_known_caldav_redirect_e2e() {
        // Validating Scenario 1: Client Auto-Discovery Redirects
        // Uses reqwest directly against the running app layer to guarantee compiling while failing cleanly.
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();

        let res = client.get("http://localhost:8080/.well-known/caldav").send().await;
        
        if let Ok(response) = res {
            assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
            let loc = response.headers().get("Location").expect("Location header missing").to_str().unwrap();
            assert_eq!(loc, "/caldav/");
        } else {
            // Will panic/fail gracefully until the server runs successfully in CI fulfilling the wiring.
            panic!("E2E Target server not reachable for well-known discovery test. Make sure the layer is hooked up.");
        }
    }

    #[tokio::test]
    async fn test_caldav_options_capabilities_e2e() {
        // Validating Scenario 2: Server Capabilities Verification
        let client = reqwest::Client::new();
        let res = client.request(reqwest::Method::OPTIONS, "http://localhost:8080/caldav/").send().await;

        if let Ok(response) = res {
            assert_eq!(response.status(), StatusCode::OK);
            let dav_header = response.headers().get("DAV").expect("DAV header missing").to_str().unwrap();
            
            // This is expected to fail because it currently returns "1, 2, calendar-access"
            assert!(dav_header.contains("1"));
            assert!(dav_header.contains("3"), "Missing CalDAV capability: 3");
            assert!(dav_header.contains("addressbook"), "Missing CardDAV capability: addressbook");
        } else {
            panic!("E2E Target server not reachable for OPTIONS capability test.");
        }
    }
}
