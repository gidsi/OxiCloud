### Priority 1: Prometheus Metrics & HTTP Health Monitoring

**Context for the team:** Self-hosters need to hook OxiCloud into their existing Grafana/Prometheus stacks to know if their instance is dropping requests from Apple Calendar or Thunderbird. The Axum instrumentation and the `/metrics` endpoint are a single cohesive feature—you cannot ship the endpoint without the data, or the data without the endpoint.

**User Story:**
As a self-hosting OxiCloud Administrator, 
I want to scrape Prometheus metrics from a `/metrics` endpoint that tracks HTTP request durations and error rates, 
so that I can monitor the performance and health of my storage server without disrupting my existing calendar and contact syncs.

**Acceptance Criteria:**

**Scenario 1: Exposing the `/metrics` endpoint**
*   **Given** the OxiCloud server is running
*   **When** a monitoring system makes an HTTP GET request to `/metrics`
*   **Then** the server responds with a `200 OK` status code
*   **And** the response `Content-Type` is `text/plain`
*   **And** the response body contains standard Prometheus-formatted metrics.

**Scenario 2: Tracking Axum HTTP routing metrics**
*   **Given** the OxiCloud server is running with metrics instrumentation active
*   **When** a client (e.g., DAVx5 or Apple Calendar) makes HTTP requests to core API routes
*   **Then** the `/metrics` endpoint output includes updated `http_requests_total` and `http_request_duration_seconds` metrics
*   **And** these metrics are properly labeled with the HTTP method, route path, and response status code.

**Scenario 3: CONSTRAINT - Instrumentation does not break WebDAV (CRITICAL)**
*   **Given** the OxiCloud server is running with the new Axum metrics middleware applied
*   **When** a client like Thunderbird makes a valid `PROPFIND` or `REPORT` request containing an XML body to a WebDAV directory
*   **Then** the server processes the request successfully and returns a `207 Multi-Status` response
*   **And** the metrics middleware does not consume, truncate, or corrupt the request/response body payload.

**Security Constraints (Security Reviewer):**
*   **Endpoint Protection:** The `/metrics` endpoint risks exposing internal system loads or timing details. If not bound to a private loopback interface by default, it must be protected via an authentication mechanism (e.g., Bearer Token or Basic Auth).
*   **Denial of Service Mitigation:** Rate-limit the `/metrics` endpoint to prevent CPU exhaustion from unauthenticated parties repeatedly forcing the server to serialize large metrics payloads.
*   **Input Sanitization:** Ensure no unsanitized user-controlled input (like query parameters or custom headers) is directly mapped to metric labels to prevent label-injection attacks.

**Architectural Constraints (Codebase/Rust Expert):**
*   **Axum 0.8.9 Body Handling:** To fulfill Scenario 3, the metrics middleware (whether via `axum::middleware::from_fn` or `tower::Service`) must **not** extract `axum::body::Body`. It must only measure the start time, await `next.run(req).await`, and calculate duration. Consuming the body will permanently break streaming WebDAV payloads.
*   **Label Cardinality:** For route path labeling, you MUST use Axum's `MatchedPath` extension (`req.extensions().get::<MatchedPath>()`). Do not use the raw URI path (`req.uri().path()`).
*   **State Management:** The Prometheus registry (e.g., via `metrics-exporter-prometheus`) must be initialized exactly once at startup and accessed safely without causing lock contention on the hot path.

**Tech Lead Synthesis & Risks:**
*   **Risk (Memory Leak / OOM):** If the `MatchedPath` constraint is ignored and raw URIs are used, the Prometheus registry will create a new label combination for every unique file or calendar event accessed (e.g., `/dav/alice/event-123.ics`), leading to unbounded memory growth and eventually an OOM kill.
*   **Synthesis:** We will use a lightweight, custom Axum middleware relying on the `metrics` crate rather than heavy third-party tracing systems. The goal is simplicity. We will bind `/metrics` to a distinct internal port or apply standard Axum auth layers to it. The middleware must stay out of the way of the HTTP body completely.
