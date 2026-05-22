# Priority 1: Seamless Calendar Setup for Standard Clients (CalDAV)

**Context:** Users currently have to dig through our documentation to find the exact `/dav/principals/users/{username}/` path to sync their calendars. We want them to just type their domain name into Thunderbird or GNOME Calendar.

**User Story:**
**As a** privacy-conscious self-hoster,
**I want** to connect my calendar client (like Thunderbird or GNOME Calendar) using only my base server URL,
**So that** I can sync my schedule quickly without having to memorize or look up complex technical server paths.

**Acceptance Criteria:**

**Scenario 1: Standard CalDAV discovery redirect**
* **Given** an operational OxiCloud server hosted at `https://cloud.example.com`
* **And** a user with a valid account
* **When** the calendar client makes an HTTP request to `https://cloud.example.com/.well-known/caldav`
* **Then** the server must respond with an HTTP redirect (e.g., 301 Moved Permanently or 302 Found)
* **And** the `Location` header must point to the root CalDAV endpoint of the server (e.g., `/dav/` or `/remote.php/dav/`)
* **And** the client must be able to successfully discover the user's calendars following that redirect.

**Scenario 2: Handling unauthenticated discovery**
* **Given** an operational OxiCloud server
* **When** a calendar client requests `/.well-known/caldav` without providing authentication headers
* **Then** the server must still successfully return the redirect to the CalDAV endpoint
* **And** only challenge for authentication (401 Unauthorized) *after* the client follows the redirect to the actual DAV path.

**Security Constraints (Security Reviewer):**
* **Unauthenticated Access:** Ensure the `/.well-known` router is completely exempt from the global application authentication middleware. No sensitive data should be exposed here.
* **Open Redirect Prevention:** The target redirect path MUST be strictly hardcoded in the handler (e.g., to `/dav/`). Under no circumstances should client input or query parameters influence the destination of the `Location` header.

**Technical Constraints (Codebase/Rust Expert):**
* **Axum Routing:** Create a new isolated `well_known_router()` in the presentation/infrastructure layer and nest it in the main application router using `Router::new().nest("/.well-known", well_known_router())`.
* **Axum Handlers:** Implement an async handler `async fn caldav_discovery() -> impl IntoResponse` utilizing `axum::response::Redirect::permanent`.

**Tech Lead Synthesis & Risks:**
* **Architectural Consistency:** This is purely an HTTP/presentation-level concern. Keeping these routes isolated from the domain layer prevents infrastructure leakage into our core application logic.
* **Risk (Caching):** Using `Redirect::permanent` (HTTP 301) means clients will cache this redirect indefinitely. If we ever decide to change our DAV namespace (e.g., from `/dav/` to `/webdav/`), clients will break. We must ensure the `/dav/` target path is a permanent architectural fixture.
