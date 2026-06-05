# Story 4: Standardize VTIMEZONE Parsing for Cross-Platform Integrity

**Priority:** Medium

**Technical Context:** 
Tokio/Axum payload streaming adjustments caused truncations in `.ics` file parsing under heavy load, specifically stripping the `VTIMEZONE` blocks.

**User Story:**
As a cross-platform user (GNOME Calendar & DAVx5), 
I want to create an event in one timezone and see it display correctly across all my devices, 
so that I don't miss important meetings due to server-side timezone offset bugs.

**Acceptance Criteria:**
*   **Given** an authenticated user scheduling an event via GNOME Calendar
*   **When** the client `PUT`s an `.ics` file containing a custom `VTIMEZONE` definition and a `DTSTART`
*   **Then** the server must parse and store the event in the database, fully preserving the timezone definition
*   **And when** DAVx5 subsequently fetches that event via `GET`, it receives the exact same `VTIMEZONE` block and offset originally provided.

**Security Constraints (Security Reviewer):**
*   **ICS Payload Validation:** Reject heavily nested or maliciously large `.ics` payloads that attempt to exhaust server memory during parsing.
*   **Sanitization:** Ensure any textual fields within the `.ics` (like `SUMMARY` or `DESCRIPTION`) are properly escaped if they are ever displayed in a web interface to prevent XSS.

**Architectural Constraints (Codebase/Rust Expert):**
*   **Async Stream Processing:** Rely on `axum::extract::Bytes` mapped with `DefaultBodyLimit` (e.g., capped at 2MB per event) for safe payload ingestion rather than manual buffer allocations.
*   **Data Fidelity:** The truncation occurs because we try to parse and re-serialize the AST of the `.ics` file. To guarantee bit-for-bit fidelity, we should store the exact raw string/bytes of the uploaded `.ics` payload into PostgreSQL (e.g., as a `TEXT` or `BYTEA` column), bypassing lossy AST conversions for storage. We only parse the payload to extract required domain indexing fields (like `DTSTART` and `UID`).

**Tech Lead Synthesis & Risks:**
*   **Synthesis:** Timezones are notoriously difficult, and trying to normalize `VTIMEZONE` rules dynamically across different client implementations is a fool's errand. We will adopt the principle of "store what the client sends." We extract only the metadata needed for filtering/queries into SQLx columns, and persist the raw `.ics` payload directly in the database.
*   **Risk:** If we don't cap the payload buffer properly via Axum boundaries, heavy concurrent `PUT` loads could spike Tokio task memory. Enforcing strict limits at the routing layer is non-negotiable.
