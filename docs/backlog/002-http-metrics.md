# Priority 2: Traffic Monitoring (HTTP Metrics with Cardinality Protection)

**Rationale:** Once uptime is secured, self-hosters need to monitor performance and traffic. However, because OxiCloud handles WebDAV requests for arbitrary files (e.g., `/remote.php/webdav/Photos/vacation.jpg`), exposing raw URLs in metrics will cause a cardinality explosion that will crash the user's Prometheus server. We must provide HTTP metrics grouped strictly by static route templates to deliver safe, reliable observability.

**User Story:**
As a Site Reliability Engineer, I want to scrape HTTP request metrics at `/metrics` grouped by static route definitions so that I can monitor OxiCloud's traffic and response times without crashing my Prometheus instance due to high label cardinality.

**Acceptance Criteria:**
*   **Scenario: Exposing standard Prometheus HTTP metrics**
    *   **Given** the OxiCloud server is processing incoming client requests
    *   **When** a monitoring system sends a `GET` request to `/metrics`
    *   **Then** the server responds with an HTTP `200 OK` status code
    *   **And** the response body is formatted as valid Prometheus plain-text
    *   **And** the response includes `http_requests_total` and `http_request_duration_seconds` metrics.
*   **Scenario: Preventing cardinality explosion on dynamic URLs**
    *   **Given** a user uploads a file via a dynamically named WebDAV URL (e.g., `PUT /remote.php/webdav/Documents/taxes.pdf`)
    *   **When** the metrics for this request are generated and exposed at `/metrics`
    *   **Then** the `route` label applied to the metric MUST be the static Axum route template (e.g., `/remote.php/webdav/*path`)
    *   **And** the raw file path (`/Documents/taxes.pdf`) MUST NOT appear in any metric label.

**Security Constraints (Security Reviewer):**
*   **Data Privacy:** URL paths often contain PII or sensitive file names. The cardinality protection requirement doubles as a security constraint. Raw URIs must not be logged in metrics.
*   **Endpoint Security:** Metrics expose application usage patterns and potentially business intelligence. The `/metrics` endpoint should ideally be served on a separate internal port, or restricted via an authentication middleware (e.g., Bearer Token).

**Architectural Constraints (Codebase/Rust Expert):**
*   **Axum:** Implement a `tower::Service` middleware or use `axum::middleware::from_fn`. To resolve the route safely, use `req.extensions().get::<axum::extract::MatchedPath>()`. 
*   **Fallback Handling:** If a request results in a `404 Not Found`, the `MatchedPath` will be empty. Group all unmatched routes under a static string (e.g., `route="UNMATCHED"`) to prevent an attacker from generating arbitrary cardinality via random 404 paths.
*   **Crates:** Use standard ecosystem crates like `metrics` and `metrics-exporter-prometheus` to avoid reinventing the wheel.

**Tech Lead Synthesis & Risks:**
*   **Risk:** Memory bloat if the `route` label logic has a flaw and falls back to `req.uri().path()`. Prometheus will ingest unbounded series and crash.
*   **Synthesis:** The use of Axum's `MatchedPath` is the perfect architectural fit here. I'm mandating a unit test specifically designed to fire 1,000 requests with random URLs at the app and asserting that the resulting Prometheus output contains exactly the expected static label sets. We will serve `/metrics` on a separate management port to satisfy security isolation cleanly.
