# Story 1: Deep Health Check for Uptime Monitoring

**Story:** 
As a System Administrator, I want a deep `/health` endpoint that actively verifies the database connection pool and memory limits so that my monitoring tools (like Uptime Kuma) can alert me the moment my server cannot handle user sync requests.

**Acceptance Criteria:**

*   **Scenario 1: System is fully healthy**
    *   **Given** OxiCloud is running normally with available database connections and stable memory
    *   **When** a monitoring system sends a GET request to `/health`
    *   **Then** the server responds with HTTP 200 OK
    *   **And** the JSON payload indicates a "healthy" status

*   **Scenario 2: Database pool exhaustion**
    *   **Given** the database connection pool is exhausted or the database is unreachable
    *   **When** a monitoring system sends a GET request to `/health`
    *   **Then** the server responds with HTTP 503 Service Unavailable
    *   **And** the JSON payload clearly indicates a database connection error

*   **Scenario 3: Memory usage exceeds safe limits**
    *   **Given** the server memory usage exceeds the configured safe operational threshold
    *   **When** a monitoring system sends a GET request to `/health`
    *   **Then** the server responds with HTTP 503 Service Unavailable
    *   **And** the JSON payload indicates a memory pressure warning

*   **Scenario 4: No interference with existing DAV client sync**
    *   **Given** the new `/health` endpoint is implemented and active
    *   **When** a user's client (e.g., DAVx5 or Thunderbird) performs a PROPFIND request to any existing DAV route
    *   **Then** the CalDAV/CardDAV sync completes successfully
    *   **And** the request is not intercepted or broken by the new health router

**Technical & Architectural Constraints (Codebase/Rust Expert):**
*   **Framework Compatibility:** Implement as an `axum 0.8.9` GET route. Ensure the route is explicitly defined to prevent falling through to CalDAV/CardDAV wildcard handlers.
*   **Application State:** Inject `sqlx::PgPool` and the memory threshold configuration via `axum::extract::State`. 
*   **Clean Architecture:** 
    *   *Infrastructure Layer:* Implement the actual OS memory read (e.g., via `/proc/self/statm` on Linux or a lightweight crate like `sysinfo` restricted to current process memory) and the `sqlx` ping query.
    *   *Application Layer:* The Axum handler should call a `HealthService` domain trait, ensuring the HTTP layer remains decoupled from OS/DB specifics.
*   **Non-Blocking DB Check:** Use `tokio::time::timeout` wrapping `sqlx::PgPool::acquire()` or a lightweight `SELECT 1` query to ensure the health check does not hang indefinitely if the pool is fully exhausted.

**Security Constraints (Security Reviewer):**
*   **Information Disclosure Mitigation:** The 503 JSON payload MUST NOT expose raw database errors, internal PostgreSQL connection strings, or full stack traces. Use generic, safe error codes (e.g., `{"status": "unhealthy", "reason": "database_exhausted"}`).
*   **Rate Limiting & Abuse:** A deep health check performs a DB query. To prevent DoS attacks exploiting the `/health` endpoint, enforce a strict rate limit (e.g., via `tower::limit::RateLimit` middleware) or implement a basic caching mechanism (e.g., cache the health status for 5-10 seconds).

**Tech Lead Risk Synthesis:**
*   **Risk - Reactor Blocking:** Using heavy system-monitoring crates (like full `sysinfo` refreshes) inside the HTTP handler can block the Tokio async executor. 
    *   *Mitigation:* If using `sysinfo`, only refresh the current process memory (`refresh_process`), or push the memory check to a background Tokio task that writes to an `Arc<AtomicUsize>` which the handler simply reads.
*   **Risk - Health Check Timeout:** If the DB is unresponsive, the health check might hang, causing monitoring tools to flag a timeout rather than a clean 503. 
    *   *Mitigation:* Enforce a strict 2-second timeout on the DB ping.
