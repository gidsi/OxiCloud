# Epic: Core CalDAV Calendar Sync (Replaces Google Calendar)
## Priority 3: Deleting Events

**Context:** Managing lifecycle completion. We must ensure that deleting an event on one device signals a deletion to all others via standard WebDAV sync tokens/ctags, bundled together so we don't end up with ghost events.

**User Story:**
**As a** daily calendar user, 
**I want to** delete canceled events in my client 
**so that** they are permanently removed from my OxiCloud server and clear out of my schedule on all my devices.

**Acceptance Criteria:**
*   **Scenario: User deletes an event**
    *   **Given** an event exists on the user's OxiCloud calendar
    *   **When** the user deletes the event in their connected CalDAV client (e.g., Apple Calendar)
    *   **Then** OxiCloud permanently deletes the `.ics` file from the server storage
    *   **And** updates the calendar's overall sync-token/ctag.
*   **Scenario: Ghost event prevention across devices**
    *   **Given** an event was deleted from the server by Device A
    *   **When** Device B (e.g., DAVx5 on mobile) requests a sync update
    *   **Then** OxiCloud accurately reports the event as deleted 
    *   **And** Device B removes the event from its local UI.

**Constraints & Synthesis:**
*   **Security Constraints (Security Reviewer):**
    *   **Authorization Enforcement:** Verify the entity making the `DELETE` request actually owns the resource. Never trust the URL alone.
    *   **Rate Limiting:** Implement strict rate limiting on `DELETE` requests to prevent automated wiping of calendars in the event of an account compromise.
*   **Architectural Constraints (Codebase/Rust Expert):**
    *   **HTTP Methods:** Implement the `DELETE` route using standard Axum `delete` routing.
    *   **Transactions:** Similar to `PUT`, `DELETE` operations must modify the event status and bump the calendar `sync-token` within a single SQLx `PgTransaction`.
*   **Tech Lead Synthesis & Risks:**
    *   *Risk:* A literal hard delete of the database row breaks WebDAV sync (`RFC 6578`). If the row is gone, the server cannot tell Device B *what* was deleted, only that a change occurred, forcing a highly expensive full-calendar resync.
    *   *Mitigation & Architectural Directive:* The AC states "permanently deletes the `.ics` file from server storage." To satisfy both the AC and the CalDAV protocol, we will separate file storage from database metadata. We will physically delete the `.ics` payload to free up disk space, but we *must* convert the DB record into a "Tombstone" (e.g., `deleted_at = NOW()`). When Device B requests a `sync-collection` REPORT, OxiCloud queries tombstones attached to the current `sync-token` to send a `<d:remove>` XML directive. Tombstones can be cleaned up via a background tokio task after 30 days.
