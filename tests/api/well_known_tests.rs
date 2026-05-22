use reqwest::{
    header::{AUTHORIZATION, LOCATION},
    redirect::Policy,
    StatusCode,
};

use crate::common::spawn_app;

fn redirect_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .expect("Failed to build reqwest client")
}

#[tokio::test]
async fn caldav_discovery_redirects_permanently_without_auth() {
    let app = spawn_app().await;
    let client = redirect_client();

    let response = client
        .get(format!("{}/.well-known/caldav", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "Expected 301 Moved Permanently. This endpoint must bypass DAV authentication middleware."
    );

    let location = response
        .headers()
        .get(LOCATION)
        .expect("Location header is completely missing from the response");

    assert_eq!(
        location.to_str().expect("Location header was not valid UTF-8"),
        "/dav/",
        "The Location header must point exactly to the root CalDAV endpoint '/dav/'"
    );
}

#[tokio::test]
async fn caldav_discovery_redirects_permanently_with_auth() {
    let app = spawn_app().await;
    let client = redirect_client();

    let response = client
        .get(format!("{}/.well-known/caldav", app.address))
        .header(AUTHORIZATION, "Bearer valid_dummy_token_for_testing")
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "Expected 301 Moved Permanently even with auth headers provided"
    );

    let location = response
        .headers()
        .get(LOCATION)
        .expect("Location header is completely missing from the response");

    assert_eq!(
        location.to_str().expect("Location header was not valid UTF-8"),
        "/dav/",
        "The Location header must point exactly to the root CalDAV endpoint '/dav/'"
    );
}

#[tokio::test]
async fn caldav_discovery_strictly_ignores_malicious_query_parameters() {
    let app = spawn_app().await;
    let client = redirect_client();

    let response = client
        .get(format!(
            "{}/.well-known/caldav?redirect=https://evil.example.com",
            app.address
        ))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "Expected 301 Moved Permanently when query parameters are present"
    );

    let location = response
        .headers()
        .get(LOCATION)
        .expect("Location header is completely missing from the response");

    assert_eq!(
        location.to_str().expect("Location header was not valid UTF-8"),
        "/dav/",
        "SECURITY FAILURE: redirect location must remain strictly hardcoded to '/dav/'"
    );
}
