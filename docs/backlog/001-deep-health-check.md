# Priority 1: Uptime Management (Deep Health Check)

**Rationale:** The most critical value for a system administrator is knowing whether the system is actually working. A simple ping isn't enough; if OxiCloud can't talk to PostgreSQL, DAVx5 and Thunderbird clients will fail to sync. We need a deep health check so orchestrators like Docker or Kubernetes can automatically restart the container if the database connection dies.

**User Story:**
As a System Administrator, I want to access a deep health check endpoint at `/health` that verifies database connectivity so that my automated orchestration tools can restart the server or alert me if OxiCloud becomes incapable of serving data.

**Acceptance Criteria:**
*   **Scenario: Health check passes when database is connected**
    *   **Given** the OxiCloud server is running 
    *   **And** the PostgreSQL database is connected and responsive
    *   **When** an automated health probe sends a `GET` request to `/health`
    *   **Then** the server responds with an HTTP `200 OK` status code
    *   **And** the response body contains a JSON payload indicating `{"status": "pass", "database": "connected"}`.
*   **Scenario: Health check fails when database is unreachable**
    *   **Given** the OxiCloud server is running
    *   **And** the PostgreSQL database connection drops or times out
    *   **When** an automated health probe sends a `GET` request to `/health`
    *   **Then** the server responds with an HTTP `503 Service Unavailable` status code
    *   **And** the response body contains a JSON payload indicating `{"status": "fail", "database": "disconnected"}`.

**Security Constraints (Security Reviewer):**
*   **Information Disclosure:** The endpoint must strictly return the defined JSON structure. Under no circumstances should it leak exact SQL connection errors, credentials, or internal IP addresses in the response if the database check fails.
*   **Availability (DoS):** While usually an internal probe, if exposed externally, this endpoint could be spammed to cause excessive DB queries. Implement a fast-path cache (e.g., caching the "up" status for 1-2 seconds) or rate-limiting if deployed at the edge.

**Architectural Constraints (Codebase/Rust Expert):**
*   **Axum:** The handler must use `axum::extract::State` to access the application's `sqlx::PgPool`. 
*   **SQLx:** Use a lightweight query: `sqlx::query("SELECT 1").execute(&pool).await`. Do not query domain tables.
*   **Tokio:** Wrap the database query in `tokio::time::timeout` to prevent the handler from hanging indefinitely if the TCP connection to Postgres silently drops.

**Tech Lead Synthesis & Risks:**
*   **Risk:** If the database connection pool is exhausted by slow requests, the health check will queue up, timeout, and fail, causing Kubernetes to restart a pod that is simply under heavy load, causing a cascading failure.
*   **Synthesis:** We must configure `sqlx::PgPoolOptions` to reserve a dedicated connection for health checks or keep the timeout strict (e.g., 2 seconds). The architecture aligns cleanly with our Axum state pattern. Ensure the route is isolated from standard API rate-limiters so orchestration probes aren't accidentally blocked.
