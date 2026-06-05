# Story 1: Restore Calendar Discovery for Desktop Clients

**Priority:** High

**Technical Context:** 
Axum 0.8.9 body-extraction changes broke the main XML PROPFIND parsing. Clients currently receive a 400 Bad Request or malformed XML during initial setup.

**User Story:**
As a Thunderbird or DAVx5 user, 
I want to connect my client to OxiCloud and have it automatically discover my existing calendars, 
so that I don't have to manually guess and configure individual calendar URLs.

**Acceptance Criteria:**
*   **Given** a provisioned user account with an existing personal calendar in the database
*   **When** a CalDAV client sends an authenticated `PROPFIND` request to the user's principal URL with `Depth: 1`
*   **Then** the server must respond with a `207 Multi-Status`
*   **And** the response body must be valid XML containing the `<caldav:calendar-home-set>` and the correct `href` to the user's calendar.

**Security Constraints (Security Reviewer):**
*   **XML Vulnerability Mitigation:** Ensure the XML parser explicitly disables External Entity processing (XXE) and guards against Billion Laughs (entity expansion) attacks.
*   **Authorization Boundary:** The `href` generation must strictly validate that the user is requesting their *own* calendar homeset or one explicitly shared with them.
*   **Payload Limits:** Enforce a strict request body size limit for PROPFIND to prevent Denial of Service (DoS) attacks.

**Architectural Constraints (Codebase/Rust Expert):**
*   **Axum 0.8.9 Body Extraction:** Use `axum::extract::Bytes` in combination with `DefaultBodyLimit` middleware rather than manually implementing body streaming, delegating the buffer handling to Axum safely.
*   **Domain Separation:** XML deserialization (using `quick-xml`) must reside in the Application layer (Axum extractors/handlers). It should map immediately into domain objects (e.g., `PropFindRequest` struct) before interacting with the Domain or Infrastructure layer.
*   **SQLx Usage:** Querying the calendar paths should rely on our existing repository pattern `CalendarRepository::get_calendars_for_user(user_id)`.

**Tech Lead Synthesis & Risks:**
*   **Synthesis:** The break in Axum 0.8.9 stems from changes to how the framework handles request bodies and traits. By utilizing `axum::extract::Bytes` and explicitly mapping the parsed XML into domain structs, we keep the HTTP concern isolated. 
*   **Risk:** If `quick-xml` configuration is loose, XXE vulnerabilities could be introduced. We must add a unit test that explicitly feeds malicious XML payloads with DTD entities to guarantee the parser rejects them safely.
