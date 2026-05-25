### Priority 2: Structured Application Tracing for Debugging

**Context for the team:** When GNOME Calendar fails to sync, our users need actionable logs. Right now, standard output is too noisy or lacks context. We need structured tracing via `tracing-subscriber`. This must be shipped securely without leaking sensitive payload data and, again, without breaking the underlying async streams used by sqlx and Axum.

**User Story:**
As a self-hosting OxiCloud Administrator, 
I want the server to output structured, context-rich logs, 
so that I can easily ingest them into my log management system (like Loki or ELK) to troubleshoot failing syncs from third-party CalDAV/CardDAV clients.

**Acceptance Criteria:**

**Scenario 1: Emitting structured traces**
*   **Given** the OxiCloud server is started with structured logging enabled via environment configuration
*   **When** an application lifecycle event, database query (via sqlx), or error occurs
*   **Then** the server outputs logs to standard output in a structured format (e.g., JSON)
*   **And** every log entry contains a timestamp, log level (INFO, WARN, ERROR, etc.), target, and relevant context fields.

**Scenario 2: Correlating logs to HTTP requests**
*   **Given** the OxiCloud server is running with structured tracing active
*   **When** a client makes an HTTP request to the server
*   **Then** a tracing span is created for the duration of the request
*   **And** all log events emitted during the processing of that request automatically include the HTTP method, request path, and a unique request ID.

**Scenario 3: CONSTRAINT - Tracing does not break WebDAV sync (CRITICAL)**
*   **Given** the OxiCloud server is running with the tracing subscriber fully instrumented across the Axum and sqlx layers
*   **When** a client performs a heavy WebDAV synchronization (e.g., uploading a large file via `PUT` or syncing a large calendar)
*   **Then** the server correctly processes the synchronization
*   **And** the tracing layer does not block the async executor, cause a timeout, or alter the expected HTTP headers and body returned to the client.

**Security Constraints (Security Reviewer):**
*   **PII & Credential Redaction:** The tracing configuration must aggressively filter out and redact sensitive HTTP headers (`Authorization`, `Cookie`, `Set-Cookie`). 
*   **No Body Logging:** Under no circumstances should HTTP request or response bodies be logged, as they contain highly sensitive personal data (CardDAV contacts, CalDAV events).
*   **Database Parameter Obfuscation:** Ensure `sqlx` logging is configured to obfuscate bind variables. Raw passwords or API keys passed to database queries must not bleed into the structured logs.

**Architectural Constraints (Codebase/Rust Expert):**
*   **Axum & tower-http Ecosystem:** Utilize `tower_http::trace::TraceLayer` to handle HTTP spans and correlation automatically. Configure the `tracing-subscriber` registry with `fmt::layer().json()`.
*   **Request Correlation ID:** Use `tower_http::request_id::SetRequestIdLayer` to generate a UUIDv4 for each incoming request, and configure the `TraceLayer` to include this ID in the top-level span context.
*   **Non-blocking Execution:** Emitting JSON to standard output in synchronous code can block tokio worker threads under load. You must wrap stdout using `tracing_appender::non_blocking` to decouple I/O from the async executor.
*   **Instrument Macros:** Use `#[tracing::instrument(skip_all)]` on domain and infrastructure functions. `skip_all` prevents accidental logging of complex arguments while still tracking the async context boundaries.

**Tech Lead Synthesis & Risks:**
*   **Risk (Executor Blocking & Timeouts):** Scenario 3 requires seamless WebDAV handling. If tracing serialization blocks a tokio thread during a large file chunk transfer, it will cause cascading timeouts for other clients. The `tracing_appender::non_blocking` constraint is absolute and non-negotiable.
*   **Risk (Log Flooding):** SQLx 0.8.6 logs at `INFO` level by default for every query. This will flood the JSON logs and degrade performance. 
*   **Synthesis:** We will implement an `EnvFilter` defaulting to `info,oxicloud=debug,sqlx=warn,tower_http=info` to maintain a high signal-to-noise ratio. The architecture relies entirely on the `tracing` crate. Developers are strictly prohibited from using `println!` or `eprintln!` moving forward to guarantee all outputs conform to the structured format.
