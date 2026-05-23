### 6. Creating and Updating Contacts (Two-way Contact Sync)
*Priority: 6 - Completes the daily driver requirement for contact management.*

**As a** self-hoster building my network, **I want to** add or edit contact details directly on my devices (e.g., Apple Contacts) **so that** my OxiCloud server is immediately updated with the latest information as my single source of truth.

**Acceptance Criteria:**
*   **Given** an existing contact named "Jane Doe" is synced to a user's Apple Contacts via OxiCloud
*   **When** the user edits "Jane Doe" in Apple Contacts to add a new email address and saves
*   **Then** the updated contact card is synced back to the OxiCloud server
*   **And** **When** the user checks their contacts on Thunderbird on their desktop
*   **Then** the new email address for "Jane Doe" is present.

**Security Constraints:**
*   **Payload Validation:** Prevent massive payload uploads. Limit vCard HTTP `PUT` requests to reasonable sizes (e.g., 2MB max, considering profile pictures might be embedded as base64).
*   **XSS Protection:** Though clients render the data, the server should ideally validate that the vCard doesn't contain glaring malicious payloads before storing it.

**Architectural Constraints:**
*   **Database Updates:** HTTP `PUT` handles creation and updates. SQLx `UPDATE` queries must bump the `sync_token` for the parent address book and update the contact's `ETag`.
*   **If-Match / If-None-Match:** Strict compliance with HTTP precondition headers is required to prevent two clients from overwriting each other's contact updates simultaneously.

**Tech Lead Synthesis & Risks:**
*   **Risk - Photo Sync Overheads:** vCards often contain base64 encoded profile pictures. Syncing these frequently can consume substantial memory and bandwidth. We must ensure our Axum stream reading and database writing utilizes Tokio's async I/O efficiently, avoiding loading massive strings into memory unnecessarily where possible.
*   **Risk - Client Quirks:** Apple Contacts and DAVx5 behave slightly differently regarding `REPORT` queries (addressbook-multiget). We must implement `REPORT` handling alongside `PROPFIND` to allow clients to efficiently fetch multiple specific vCards in a single request.
