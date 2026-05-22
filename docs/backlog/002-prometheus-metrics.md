# Story 2: Prometheus Metrics for Proactive Resource Management

**Story:** 
As a System Administrator, I want a read-only `/metrics` endpoint exposing Prometheus-formatted data so that I can visualize OxiCloud's resource usage in Grafana and proactively scale my server before a crash disrupts users' calendar and contact syncs.

**Acceptance Criteria:**

*   **Scenario 1: Standard Prometheus scraping**
    *   **Given** OxiCloud is running and processing data
    *   **When** a Prometheus scraper sends a GET request to `/metrics`
    *   **Then** the server responds with HTTP 200 OK
    *   **And** the response body contains plain-text, valid Prometheus-formatted metrics (including `# HELP` and `# TYPE` definitions)

*   **Scenario 2: Specific metrics are exposed**
    *   **Given** OxiCloud is running and processing data
    *   **When** a Prometheus scraper fetches `/metrics`
    *   **Then** the response specifically includes a metric for current active database connections
    *   **And** the response specifically includes a metric for current memory usage

*   **Scenario 3: Endpoint is strictly read-only**
    *   **Given** the `/metrics` endpoint is active
    *   **When** a client sends a POST, PUT, PATCH, or DELETE request to `/metrics`
    *   **Then** the server responds with HTTP 405 Method Not Allowed
    *   **And** no server state is changed

*   **Scenario 4: No interference with CalDAV modifications**
    *   **Given** the `/metrics` endpoint is implemented and active
    *   **When** an end-user creates a new calendar event via Apple Calendar or GNOME Calendar (sending a PUT request to their DAV calendar URL)
    *   **Then** the event is successfully saved to the database
    *   **And** the DAV request is routed correctly without any interference from the metrics endpoint routes

**Technical & Architectural Constraints (Codebase/Rust Expert):**
*   **Libraries:** Utilize the `metrics` and `metrics-exporter-prometheus` crates. 
*   **SQLx 0.8.6 Integration:** Leverage the built-in pool inspection methods (`pool.size()`, `pool.num_idle()`) to populate the database gauges.
*   **Background Polling:** Create a background `tokio::spawn` task that periodically (e.g., every 5 seconds) polls the memory usage and SQLx pool statistics, updating the metrics registry. Avoid polling these heavily on every scrape request to keep the `/metrics` endpoint ultra-fast.
*   **Axum 0.8.9 Routing:** 
    *   Register `axum::routing::get` for `/metrics` at the highest level of the router hierarchy to ensure exact matching. Axum will automatically handle Scenario 3 (HTTP 405 Method Not Allowed) if only `get` is specified.

**Security Constraints (Security Reviewer):**
*   **Access Control:** Prometheus metrics reveal internal application behavior, traffic patterns, and resource usage over time. This endpoint MUST be secured. Either require a static Bearer Token (via `Authorization` header) or restrict access at the Axum routing level to trusted internal IP ranges/subnets.
*   **Data Leakage in Labels:** Ensure absolutely no Personally Identifiable Information (PII) is used in metric labels. Do not include User IDs, calendar names, or event IDs as label values, as this causes both privacy breaches and cardinality explosions.

**Tech Lead Risk Synthesis:**
*   **Risk - Cardinality Explosion:** If developers log metrics with highly variable labels (like request URLs containing specific DAV resource paths), memory usage will skyrocket. 
    *   *Mitigation:* Enforce strict static labels. All HTTP route metrics must use matched route patterns (e.g., `/dav/:user/:calendar`), NOT the raw URL path.
*   **Risk - Route Shadowing:** CalDAV clients are notoriously aggressive with PROPFIND requests to root paths. 
    *   *Mitigation:* Write an automated integration test verifying that a `PROPFIND /metrics` returns the proper DAV response (or 405), and `GET /metrics` returns the Prometheus payload. This ensures our Axum route nesting rules are correct and robust against regression.
