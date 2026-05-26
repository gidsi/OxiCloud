# Epic: Core CalDAV Calendar Sync (Replaces Google Calendar)
## Priority 1: Basic Calendar Connection & Read-Only Sync

**Context:** This is the absolute core vertical slice. We are bundling Authentication, Endpoint Discovery (`/.well-known/caldav`), and Read operations (PROPFIND/REPORT). If we split auth or discovery into a separate story, our automated client integration tests will fail, causing development deadlocks. Build it as one cohesive slice.

**User Story:**
**As a** privacy-conscious self-hoster, 
**I want to** connect my standard calendar client (like Thunderbird or DAVx5) to OxiCloud 
**so that** I can securely view my existing server-side calendar events without relying on Big Tech.

**Acceptance Criteria:**
*   **Scenario: Client successfully authenticates and discovers the calendar endpoint**
    *   **Given** the user has an active OxiCloud account with an existing calendar
    *   **When** they enter their OxiCloud username, password, and root server URL into their CalDAV client (e.g., Apple Calendar)
    *   **Then** OxiCloud authenticates the request
    *   **And** successfully routes the client to their specific calendar via standard CalDAV discovery (`/.well-known/caldav`)
*   **Scenario: Client fetches existing events**
    *   **Given** the client is authenticated and has discovered the calendar
    *   **When** the client initiates a sync request
    *   **Then** OxiCloud returns all existing `.ics` event files from the user's storage
    *   **And** the client successfully displays them in the UI.
*   **Scenario: Security and Data Isolation constraint**
    *   **Given** a user is authenticated via their CalDAV client
    *   **When** the client requests the calendar list
    *   **Then** OxiCloud strictly returns ONLY the calendars belonging to that authenticated user, blocking access to any other user's data on the instance.

**Constraints & Synthesis:**
*   **Security Constraints (Security Reviewer):**
    *   **Authentication:** Must support Basic Auth over TLS strictly. Validate passwords using our existing `Argon2id` hashed auth mechanism.
    *   **XML XXE Prevention:** The `PROPFIND` and `REPORT` request bodies are XML. The parser must strictly disable DTDs/External Entities to prevent XXE injection.
    *   **Data Isolation:** Inject the authenticated `User_ID` into Axum’s request extensions. The Infrastructure layer must append `WHERE user_id = $1` to every SQL query. Path traversal vulnerabilities must be prevented by rigorously validating the requested Calendar ID against the user context.
*   **Architectural Constraints (Codebase/Rust Expert):**
    *   **Axum 0.8.9 Routing:** CalDAV uses custom HTTP verbs (`PROPFIND`, `REPORT`, `OPTIONS`). Use Axum's `MethodFilter` or standard `on(MethodFilter::all(), handler)` and route internally, since these aren't standard `GET`/`POST`.
    *   **State Management:** DB connection pools and configurations must be passed using `axum::extract::State`.
    *   **Parsing:** Use `quick-xml` for serialization/deserialization. `serde-xml-rs` is unmaintained and known to panic on complex WebDAV structures. Keep XML structs purely in the Application/Adapter layer; map them to Clean Domain entities (`Calendar`, `Event`) before passing them to core logic.
    *   **Database:** Use SQLx 0.8.6 `query_as!` macros for strict compile-time checking of the PostgreSQL schema.
*   **Tech Lead Synthesis & Risks:**
    *   *Risk:* Memory bloat from parsing massive `PROPFIND` payload requests.
    *   *Mitigation:* Implement an `axum::extract::DefaultBodyLimit` (e.g., 2MB) specifically on CalDAV routes. 
    *   *Design Directive:* The `/.well-known/caldav` route should just return an HTTP 301/308 Redirect to the user's principal URL (e.g., `/caldav/v1/principals/{user_id}/`). This adheres to RFC 4791 and keeps endpoint routing clean.
