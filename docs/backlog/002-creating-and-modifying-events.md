# Epic: Core CalDAV Calendar Sync (Replaces Google Calendar)
## Priority 2: Creating and Modifying Events

**Context:** Now that the user can see their calendar, they need to manage their schedule. We are bundling creation (PUT), modification, and conflict resolution (ETags). You cannot build event creation without ETags, otherwise, users editing the same event on mobile and desktop will corrupt their schedule.

**User Story:**
**As a** daily calendar user, 
**I want to** create and edit events directly from my calendar app (e.g., GNOME Calendar) 
**so that** my schedule changes are securely saved to OxiCloud and instantly available across all my synced devices.

**Acceptance Criteria:**
*   **Scenario: User creates a new event**
    *   **Given** the user's client is successfully synced with OxiCloud
    *   **When** the user creates and saves a new event in their local calendar app
    *   **Then** the client securely transmits the valid `.ics` payload to OxiCloud
    *   **And** OxiCloud saves the event to the user's storage backend
    *   **And** OxiCloud assigns and returns a unique ETag to the client for future sync tracking.
*   **Scenario: User updates an existing event**
    *   **Given** the user has an existing event synced to their client
    *   **When** the user changes the event's details (e.g., time, title) and saves
    *   **Then** OxiCloud updates the event payload in storage
    *   **And** OxiCloud generates a *new* ETag for the updated event to signal the change to other devices.
*   **Scenario: Preventing sync conflicts (Constraint)**
    *   **Given** an event exists on OxiCloud with a specific ETag
    *   **When** a client attempts to update the event using an outdated ETag (meaning another device already changed it)
    *   **Then** OxiCloud rejects the update with a `412 Precondition Failed` error
    *   **And** forces the client to fetch the newest version before making changes.

**Constraints & Synthesis:**
*   **Security Constraints (Security Reviewer):**
    *   **Payload Validation:** Even though `.ics` is text, reject payloads exceeding 1MB to prevent DoS. Verify that the `Content-Type` is strictly `text/calendar`. 
    *   **ETag Unpredictability:** ETags must not be predictable sequential IDs. Use a strong hash (e.g., SHA-256 of the `.ics` payload bytes + updated timestamp) wrapped in double quotes as required by spec.
*   **Architectural Constraints (Codebase/Rust Expert):**
    *   **Header Extraction:** Utilize Axum extractors to explicitly parse the `If-Match` and `If-None-Match` headers.
    *   **Database Transactions:** A `PUT` request updates an event but must *also* update the parent Calendar's `ctag`/`sync-token`. This requires a single atomic transaction. Use SQLx `PgTransaction`.
*   **Tech Lead Synthesis & Risks:**
    *   *Risk:* Race conditions when two clients PUT to the same event simultaneously.
    *   *Mitigation (Optimistic Concurrency):* Enforce ETags at the database level. Add an `etag` column to the `events` table. The SQLx update query must be: `UPDATE events SET payload = $1, etag = $2 WHERE id = $3 AND etag = $4`. If `rows_affected == 0`, immediately rollback the transaction and yield `412 Precondition Failed`. This isolates synchronization logic deep in the Infrastructure layer, preventing dirty writes.
