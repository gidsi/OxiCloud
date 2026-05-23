### 2. Creating and Modifying Events (Two-way Calendar Sync)
*Priority: 2 - The core daily interaction. A calendar is useless if you can't add to it on the go.*

**As a** busy professional, **I want to** create and edit calendar events from my mobile client (via DAVx5) **so that** my schedule changes are immediately saved to my self-hosted server and reflect across all my other devices.

**Acceptance Criteria:**
*   **Given** a user has their OxiCloud calendar synced to an Android device via DAVx5
*   **When** the user creates a new event titled "Dentist Appointment" for tomorrow at 10:00 AM on their phone
*   **Then** the Android client successfully syncs the new event to the OxiCloud server
*   **And** **When** the user opens GNOME Calendar on their synced Linux desktop
*   **Then** the "Dentist Appointment" event appears on the desktop calendar without requiring manual server-side intervention.

**Security Constraints:**
*   **Input Validation:** Strictly enforce `Content-Length` limits for `.ics` payloads. Validate that the payload is valid iCalendar format to prevent injection or malicious data storage.
*   **Resource Ownership Check:** Ensure the user attempting the `PUT` request has write permissions to the specified calendar collection.

**Architectural Constraints:**
*   **Axum State & SQLx:** Use `axum::extract::State` to inject the PostgreSQL connection pool. Use SQLx 0.8.6 `INSERT ... ON CONFLICT DO UPDATE` or transaction blocks for safe event creation/modification.
*   **Storage Strategy:** Store the raw `.ics` data in the database rather than completely deconstructing it. Extract and index only the essential metadata (UID, DTSTART, DTEND) into PostgreSQL columns for fast querying, keeping the raw `.ics` as a `TEXT` or `BYTEA` column to avoid losing custom client properties.
*   **ETags:** Generate and store an `ETag` (hash or version UUID) for every event update.

**Tech Lead Synthesis & Risks:**
*   **Risk - Concurrency:** Clients rely heavily on `If-Match` headers for updates to avoid overwriting changes from other devices. We must validate the ETag in the `PUT` request against the database ETag within a SQLx transaction.
*   **Risk - Timezones and Recurring Events (RRULE):** Computing recurring event instances on the server is complex. By storing the raw `.ics`, we offload the rendering logic to the client, which is the standard CalDAV approach.
