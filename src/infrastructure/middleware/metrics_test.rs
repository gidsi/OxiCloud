#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        routing::{any, get},
        middleware::from_fn,
        Router,
    };
    use tower::ServiceExt;
    
    // Placeholder imports for the components that developers will implement
    use crate::infrastructure::middleware::metrics::{metrics_middleware, get_metrics};

    #[tokio::test]
    async fn test_metrics_middleware_prevents_unbounded_label_cardinality() {
        let app = Router::new()
            .route("/api/users/{id}", get(|| async { "user data" }))
            .route("/metrics", get(get_metrics))
            .layer(from_fn(metrics_middleware));

        // Make requests with varying dynamic paths to ensure labels are consolidated
        let req1 = Request::builder()
            .method(Method::GET)
            .uri("/api/users/uuid-1234")
            .body(Body::empty())
            .unwrap();
        let res1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(res1.status(), StatusCode::OK);

        let req2 = Request::builder()
            .method(Method::GET)
            .uri("/api/users/uuid-5678")
            .body(Body::empty())
            .unwrap();
        let res2 = app.clone().oneshot(req2).await.unwrap();
        assert_eq!(res2.status(), StatusCode::OK);

        // Fetch metrics to verify MatchedPath was used
        let metrics_req = Request::builder()
            .method(Method::GET)
            .uri("/metrics")
            .body(Body::empty())
            .unwrap();
        let metrics_res = app.clone().oneshot(metrics_req).await.unwrap();
        let body_bytes = axum::body::to_bytes(metrics_res.into_body(), usize::MAX).await.unwrap();
        let body_text = String::from_utf8_lossy(&body_bytes);

        // Architectural Constraint: Assert the generalized route path is present
        assert!(
            body_text.contains(r#"path="/api/users/{id}""#), 
            "Expected generalized MatchedPath label"
        );
        
        // Architectural Constraint: Assert raw URIs are NOT present (prevents memory leak / OOM)
        assert!(!body_text.contains("uuid-1234"), "Raw URI should not be used as a metric label");
        assert!(!body_text.contains("uuid-5678"), "Raw URI should not be used as a metric label");
    }

    #[tokio::test]
    async fn test_metrics_middleware_sanitizes_inputs() {
        let app = Router::new()
            .route("/api/search", get(|| async { "search results" }))
            .route("/metrics", get(get_metrics))
            .layer(from_fn(metrics_middleware));

        // Request with potentially malicious label injections in query and headers
        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/search?query=exploit_label_injection")
            .header("User-Agent", "malicious_ua")
            .header("X-Forwarded-For", "evil_ip")
            .body(Body::empty())
            .unwrap();

        app.clone().oneshot(req).await.unwrap();

        let metrics_req = Request::builder()
            .method(Method::GET)
            .uri("/metrics")
            .body(Body::empty())
            .unwrap();
            
        let metrics_res = app.clone().oneshot(metrics_req).await.unwrap();
        let body_bytes = axum::body::to_bytes(metrics_res.into_body(), usize::MAX).await.unwrap();
        let body_text = String::from_utf8_lossy(&body_bytes);

        // Security Constraint: No user input map directly to labels
        assert!(!body_text.contains("exploit_label_injection"), "Metrics must not expose unsanitized query parameters");
        assert!(!body_text.contains("malicious_ua"), "Metrics must not expose unsanitized headers");
    }

    #[tokio::test]
    async fn test_metrics_middleware_does_not_consume_body() {
        // Axum Body Handling Constraint: Middleware must not consume or extract `axum::body::Body`
        let app = Router::new()
            .route("/dav/upload", any(|body: String| async move { body }))
            .layer(from_fn(metrics_middleware));

        let stream_payload = "test streaming payload chunk";

        let req = Request::builder()
            .method("PUT")
            .uri("/dav/upload")
            .body(Body::from(stream_payload))
            .unwrap();

        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        
        // If the middleware accidentally extracts the body in `req.into_parts()`, the downstream handler gets an empty body.
        assert_eq!(
            body_bytes.as_ref(), 
            stream_payload.as_bytes(), 
            "Middleware consumed or corrupted the body payload"
        );
    }
}
