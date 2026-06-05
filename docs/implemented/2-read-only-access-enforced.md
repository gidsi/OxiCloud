# 2. PRIORITY: HIGH (FAST-FOLLOW REGRESSION CHECK)
**Title:** Read-Only Access Enforced for Non-Owned/Shared Calendars

**Description:** 
When we fix the authorization logic above, we must ensure we don't swing the pendulum too far and accidentally grant `D:write` privileges to *everyone*. Users who are subscribed to another user's public or read-only calendar must not be able to edit it.

**User Story:**
As an OxiCloud user sharing calendars, 
I want to ensure that users with view-only access cannot edit my calendar events 
so that my personal schedule remains secure from unauthorized modifications.

**Acceptance Criteria:**
*   **Scenario 1: Read-only privilege enforcement in PROPFIND**
    *   **Given** User A owns a calendar and User B only has read access to it
    *   **When** User B's CalDAV client performs a `PROPFIND` request on User A's calendar
    *   **Then** the XML response MUST include `<D:read/>`
    *   **And** the XML response MUST NOT include `<D:write/>` in the `<D:current-user-privilege-set>` block.
*   **Scenario 2: Server-side rejection of unauthorized edits**
    *   **Given** User B is viewing User A's read-only calendar in a client like GNOME Calendar or Thunderbird
    *   **When** User B attempts to push an event creation or modification via a `PUT` request to that calendar
    *   **Then** the OxiCloud Axum server must reject the operation
    *   **And** it must return a `403 Forbidden` status code to protect domain boundaries.

**Constraints & Specialist Input:**

*   **Architectural Constraints (Codebase/Rust Expert):**
    *   **SQLx Patterns:** When fetching the calendar via `sqlx 0.8.6`, the query should cleanly map the `owner_id` (UUID in Postgres) to our domain struct without relying on raw string casts in SQL.
    *   **Error Handling:** We must map domain-level authorization errors (e.g., `DomainError::InsufficientPermissions`) to Axum's HTTP responses using the `axum::response::IntoResponse` trait, automatically yielding a `StatusCode::FORBIDDEN`.

*   **Security Constraints (Security Reviewer):**
    *   **IDOR Prevention (Resource Enumeration):** If User B does not even have *read* access to User A's calendar, the server MUST return `404 Not Found`, not `403 Forbidden`. Only return `403 Forbidden` if User B has *read* access but is attempting an unauthorized *write*.
    *   **Denial of Service (DoS):** Prevent malicious users from bypassing quota/limits. Enforce Axum's `DefaultBodyLimit` on `PUT` requests to block massive `.ics` file uploads meant to exhaust server memory.

*   **Tech Lead Synthesis & Risk Analysis:**
    *   **Risk:** The Acceptance Criteria only explicitly mentions the HTTP `PUT` method. In CalDAV, modifications can also happen via `DELETE` (removing events) and `PROPPATCH` (modifying properties). If we only secure `PUT`, we have an active vulnerability.
    *   **Mitigation:** The authorization check MUST NOT be implemented purely inside the `PUT` handler. We will implement an Axum middleware or centralize the domain authorization check (`ensure_write_access(&user, &calendar)?`) to be invoked at the top of *all* state-mutating CalDAV handlers (`PUT`, `DELETE`, `MKCALENDAR`, `PROPPATCH`). I will personally review this PR to ensure no mutating verb is left unprotected.
