# Priority 3: Resource Planning (Memory Usage Metrics)

**Rationale:** Rust is highly memory-efficient, but large file uploads or complex calendar syncs can still spike memory. Self-hosters need to monitor RAM usage to set appropriate container limits and avoid Out-Of-Memory (OOM) kills that corrupt in-flight syncs. This is our final slice for the epic.

**User Story:**
As a System Administrator, I want to view current memory usage statistics on the `/metrics` endpoint so that I can configure alerts for memory leaks and appropriately size my server before OxiCloud crashes.

**Acceptance Criteria:**
*   **Scenario: Exposing process memory metrics**
    *   **Given** the OxiCloud server is running and Prometheus metrics are enabled
    *   **When** a monitoring system sends a `GET` request to `/metrics`
    *   **Then** the response includes a gauge metric for process memory usage (e.g., `process_resident_memory_bytes`)
    *   **And** the value accurately reflects the current memory consumption of the OxiCloud process.
*   **Scenario: Safe handling of system limits**
    *   **Given** the OxiCloud process is nearing its configured container memory limits
    *   **When** the `/metrics` endpoint is scraped
    *   **Then** the endpoint continues to return a `200 OK` rapidly (under 50ms) without allocating significant new memory to generate the response.

**Security Constraints (Security Reviewer):**
*   **Information Leakage:** As with standard metrics, knowing memory thresholds allows an attacker to optimize a Resource Exhaustion attack. Strict adherence to exposing this only on the internal management port is required.

**Architectural Constraints (Codebase/Rust Expert):**
*   **Crates:** Utilize standard process exporters (like `metrics-process`) which are cross-platform compatible and hook into the Prometheus exporter smoothly. 
*   **Concurrency:** Querying OS memory stats (e.g., reading `/proc/self/statm` on Linux) can be a blocking system call. This must NOT happen directly in the Axum handler when a scrape occurs.

**Tech Lead Synthesis & Risks:**
*   **Risk:** If reading OS memory metrics blocks the tokio thread during a scrape, we introduce latency and potential executor stalling under heavy scrape loads. Furthermore, generating a huge Prometheus text response requires allocation, which might fail if the system is already near OOM.
*   **Synthesis:** To keep the `/metrics` endpoint OOM-safe and non-blocking, we will spawn a background Tokio task on server startup. This task will wake up every 5-10 seconds, query the OS memory safely using `tokio::task::spawn_blocking`, and update a global metrics gauge. The Axum handler will simply render the pre-computed registry state, requiring minimal allocation and zero blocking I/O on the critical path.
