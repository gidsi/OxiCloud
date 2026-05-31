# 1. PRIORITY: CRITICAL (BLOCKER)
**Title:** Calendar Owner Can Edit Events via CalDAV Clients (Apple Calendar, DAVx5, Thunderbird)

**Description:** 
Currently, users are blocked from editing their own calendars because our CalDAV adapter fails to grant them write privileges. The root cause is a hardcoded string comparison (`calendar.owner_id == "current_user_id"`) in `src/application/adapters/caldav_adapter.rs` instead of correctly matching the actual user UUIDs. We need to fix this authorization check and guarantee it never regresses.

**User Story:**
As a privacy-conscious self-hoster, 
I want to create, edit, and delete events from my native CalDAV clients (Apple Calendar, DAVx5, Thunderbird) 
so that I can manage my schedule seamlessly without having to log into a web interface.

**Acceptance Criteria:**
*   **Scenario 1: Automated E2E verification of CalDAV Privileges**
    *   **Given** a user is authenticated and owns a calendar in OxiCloud
    *   **When** an automated e2e/integration test simulates a client making a `PROPFIND` request to the calendar endpoint
    *   **Then** the `PROPFIND` XML response MUST include both `<D:read/>` and `<D:write/>` inside the `<D:current-user-privilege-set>` block
    *   **And** the test must pass consistently in the CI pipeline against our Axum test server.
*   **Scenario 2: Real-world client event creation (Apple Calendar/DAVx5)**
    *   **Given** the user has connected their OxiCloud account to Apple Calendar or DAVx5
    *   **When** the user creates a new calendar event from their device
    *   **Then** the event is successfully synced to the OxiCloud server via a `PUT` request without throwing a read-only or forbidden error.
*   **Scenario 3: Resolving the UUID Authorization Logic**
    *   **Given** the system is building the PROPFIND XML response in the application layer
    *   **When** it evaluates if the current user has write access
    *   **Then** it must successfully compare the authenticated user's `uuid::Uuid` against the `calendar.owner_id` (also a `Uuid`)
    *   **And** the legacy hardcoded `"current_user_id"` string check must be completely removed.

**Constraints & Specialist Input:**

*   **Architectural Constraints (Codebase/Rust Expert):**
    *   **Axum State & Extractors:** The authenticated user's `uuid::Uuid` MUST be retrieved via a strongly-typed Axum Extractor (e.g., `Extension<AuthenticatedUser>`) rather than manually parsed in the handler. 
    *   **Domain Isolation:** Do NOT perform the UUID comparison directly in the XML serialization logic. The domain entity (`Calendar`) should expose a method like `fn permissions_for(&self, user_id: &Uuid) -> PrivilegeSet`, which the application layer (`caldav_adapter.rs`) then maps to the `<D:current-user-privilege-set>` XML nodes.
    *   **Testing:** E2E testing must utilize Axum's `tower::ServiceExt::oneshot` to simulate the HTTP requests in memory without needing to bind to a live port, keeping tests fast and asynchronous via `tokio`.

*   **Security Constraints (Security Reviewer):**
    *   **Authentication Enforcement:** Ensure the CalDAV Axum endpoints are protected by our auth middleware. Missing or invalid credentials MUST immediately return `401 Unauthorized` before reaching the UUID logic.
    *   **Constant-time Execution:** While `uuid::Uuid` equality (`==`) is sufficient here (it compares internal byte arrays), ensure we aren't leaking timing details on authorization failures.

*   **Tech Lead Synthesis & Risk Analysis:**
    *   **Risk:** Modifying XML generation can easily break strict CalDAV clients if namespaces (`xmlns:D="DAV:"`) get mangled. 
    *   **Mitigation:** We must use automated snapshot testing for the XML output. I'm enforcing a strict boundary: the Domain dictates *if* they can write, the Adapter purely translates `write: true` to `<D:write/>`. This ensures domain invariants are decoupled from WebDAV idiosyncrasies.
