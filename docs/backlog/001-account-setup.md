### 1. Account Setup and Calendar Discovery (Read-only Sync)
*Priority: 1 (Highest) - The absolute foundation. Users must be able to authenticate and see their existing cloud schedule.*

**As a** privacy-conscious self-hoster, **I want to** connect my desktop calendar client (e.g., Thunderbird or Apple Calendar) to OxiCloud using my server URL and credentials **so that** I can view my centralized schedule without relying on Big Tech ecosystems.

**Acceptance Criteria:**
*   **Given** an OxiCloud user account exists with a default calendar containing at least one future event
*   **When** the user adds a new CalDAV network calendar in Thunderbird using their OxiCloud URL, username, and password
*   **Then** the client successfully authenticates without errors
*   **And** the client automatically discovers the default calendar
*   **And** the existing OxiCloud event is displayed correctly on the Thunderbird calendar grid.

**Security Constraints:**
*   **Authentication:** Must enforce Basic Authentication over HTTPS. Password hashing must be validated using Argon2id.
*   **Rate Limiting:** Implement brute-force protection on the authentication middleware to rate-limit failed login attempts.
*   **Authorization:** Ensure strict tenant isolation. The `PROPFIND` handler must only return properties for calendars owned by the authenticated user principal.

**Architectural Constraints:**
*   **WebDAV/Axum Routing:** Implement Axum 0.8.9 routing for `.well-known/caldav` (redirecting to the actual principal URL) and handle `PROPFIND` WebDAV methods using custom Axum method extractors.
*   **XML Parsing/Serialization:** Use a robust, non-blocking XML crate (e.g., `quick-xml`) for reading and writing WebDAV multistatus XML responses.
*   **Clean Architecture:** Implement a `CalDavService` in the application layer that handles protocol translation, relying on a `CalendarRepository` trait in the domain layer. The SQLx 0.8.6 implementation will live in the infrastructure layer.

**Tech Lead Synthesis & Risks:**
*   **Risk - XML DoS:** XML parsing is susceptible to "Billion Laughs" or massive payload attacks. We must configure the Axum payload extractor with strict size limits (e.g., via `axum::extract::DefaultBodyLimit`).
*   **Risk - RFC 4791 Complexity:** CalDAV discovery requires a specific hierarchy (Principal URL -> Calendar Home Set -> Calendar). Clients are notoriously strict. We must test against Apple Calendar and Thunderbird specifically during development.
