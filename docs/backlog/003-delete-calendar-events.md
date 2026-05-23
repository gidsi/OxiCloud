### 3. Deleting Calendar Events
*Priority: 3 - Completes the basic CRUD lifecycle for calendar management.*

**As a** self-hoster managing an evolving schedule, **I want to** delete canceled events from my preferred client (e.g., GNOME Calendar) **so that** the event is permanently removed from my server and disappears from my other synced devices.

**Acceptance Criteria:**
*   **Given** an existing "Weekly Sync" event is stored on OxiCloud and visible in GNOME Calendar
*   **When** the user deletes the "Weekly Sync" event from within GNOME Calendar
*   **Then** the client communicates the deletion to the OxiCloud server
*   **And** the server permanently removes the event from the calendar data
*   **And** the event disappears from the user's Apple Calendar on their iPhone upon its next background sync.

**Security Constraints:**
*   **Authorization:** The `DELETE` handler must verify that the user requesting the deletion is the owner of the resource.

**Architectural Constraints:**
*   **WebDAV Sync-Token:** To support efficient syncing (RFC 6578 WebDAV Sync), we cannot simply perform a hard SQL `DELETE`. We must implement a "tombstone" pattern.
*   **Database Schema:** Add a `deleted_at` timestamp or a separate `tombstones` table. Ensure the `sync_token` sequence on the calendar collection is incremented upon deletion.

**Tech Lead Synthesis & Risks:**
*   **Risk - Battery Drain on Mobile:** If we do not implement WebDAV Sync tokens and tombstones correctly, mobile clients will be forced to do a full `PROPFIND` of every event to determine what was deleted. This destroys mobile battery life. We must design the SQLx PostgreSQL schema to support `sync-token` natively from the beginning.
