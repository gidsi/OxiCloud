use crate::helpers::spawn_app;
use reqwest::StatusCode;

#[tokio::test]
async fn caldav_redirects_to_dav() {
    let app = spawn_app().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let response = client
        .get(&format!("{}/.well-known/caldav", app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response.headers().get("Location").unwrap().to_str().unwrap(),
        "/dav/"
    );
}

#[tokio::test]
async fn well_known_carddav_redirects_with_relative_path() {
    let app = spawn_app().await;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let response = client
        .get(&format!("{}/.well-known/carddav", app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(
        response.status().is_redirection(),
        "Expected a redirect status code (3xx), got {}",
        response.status()
    );

    let location = response
        .headers()
        .get("Location")
        .expect("Location header is missing in the redirect response")
        .to_str()
        .expect("Location header contains invalid characters");

    assert_eq!(
        location,
        "/dav/",
        "Location MUST be exactly '/dav/' to preserve the connection's scheme and prevent proxy spoofing vulnerabilities"
    );
}

#[tokio::test]
async fn well_known_carddav_rejects_non_get_requests() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    let response = client
        .post(&format!("{}/.well-known/carddav", app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(
        response.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "Expected 405 Method Not Allowed, well-known discovery MUST reject POST requests"
    );
}

#[tokio::test]
async fn well_known_carddav_does_not_trust_forwarded_headers() {
    let app = spawn_app().await;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let response = client
        .get(&format!("{}/.well-known/carddav", app.address))
        .header("X-Forwarded-Host", "evilhacker.com")
        .header("X-Forwarded-Proto", "http")
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(
        response.status().is_redirection(),
        "Expected a redirect status code"
    );

    let location = response
        .headers()
        .get("Location")
        .expect("Location header is missing")
        .to_str()
        .unwrap();

    assert_eq!(
        location,
        "/dav/",
        "Location MUST remain '/dav/' even if X-Forwarded headers are present, to prevent Open Redirects"
    );
}
