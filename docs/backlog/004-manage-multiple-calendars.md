### 4. Managing Multiple Calendars (Work vs. Personal)
*Priority: 4 - Essential for power users who categorize their lives.*

**As a** privacy-conscious user managing multiple life areas, **I want to** sync multiple distinct calendars (e.g., "Work" and "Personal") to my client **so that** I can color-code and organize my schedule without mixing contexts.

**Acceptance Criteria:**
*   **Given** an OxiCloud user has created two separate calendars on the server named "Work" and "Personal"
*   **When** the user configures their OxiCloud account in Apple Calendar
*   **Then** Apple Calendar discovers and lists both "Work" and "Personal" as independent toggleable calendars
*   **And** the user can select which specific calendar a new event should be saved to.

**Security Constraints:**
*   **URL Path Isolation:** Directory traversal vulnerabilities must be mitigated. Extract parameters for `{user_id}` and `{calendar_id}` safely in Axum and validate that `{calendar_id}` belongs strictly to `{user_id}` in every query.

**Architectural Constraints:**
*   **Relational Model:** Domain mapping of `User (1) -> (N) Calendar (1) -> (N) Event`. SQLx queries must enforce these joins.
*   **Dynamic PROPFIND:** The `PROPFIND` handler at the user's Calendar Home Set (`/dav/calendars/{user}/`) must dynamically query the DB and construct XML `<response>` blocks for every calendar the user owns.

**Tech Lead Synthesis & Risks:**
*   **Risk - Axum Router Conflicts:** Handling wildcard or deep paths in Axum (`/dav/calendars/:user/:calendar/:event.ics`) requires precise routing definitions to avoid shadowing other endpoints.
*   **Design Note:** We need to support `MKCOL` (or `MKCALENDAR` from RFC 4791) to allow clients to create new calendars natively, saving them to the PostgreSQL DB with a generated UUID and an initial sync token.
