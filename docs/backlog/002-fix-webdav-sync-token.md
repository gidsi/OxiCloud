# Story 2: Fix WebDAV Sync-Token Updates for Apple Calendar

**Priority:** High

**Technical Context:** 
SQLx 0.8.6 introduced stricter type checking, which is causing our `sync-token` database updates to silently fail or panic. Apple Calendar relies aggressively on `sync-collection` reports and hangs/drains battery doing full HTTP syncs without it.

**User Story:**
As an Apple Calendar user, 
I want my calendar to reliably fetch only newly changed events using WebDAV Sync, 
so that my client syncs instantly without draining my device's battery.

**Acceptance Criteria:**
*   **Given** a user has synchronized their calendar and possesses a valid `sync-token`
*   **When** the client sends a `sync-collection` REPORT request using the provided token
*   **Then** the server must query PostgreSQL and return a `207 Multi-Status` containing *only* the `href`s of events created, modified, or deleted since that token was issued
*   **And** the response must include a newly generated `<d:sync-token>` that is successfully persisted to the database.

**Security Constraints (Security Reviewer):**
*   **Token Predictability:** The `sync-token` must be cryptographically secure (e.g., UUIDv4 or a high-entropy hash) to prevent enumeration of sync states by malicious actors.
*   **Data Isolation:** The database query fetching the diff MUST scope the query strictly to the authenticated `user_id` alongside the `sync-token`.

**Architectural Constraints (Codebase/Rust Expert):**
*   **SQLx 0.8.6 Type Matching:** The panic/silent failure is due to type mismatching in the updated SQLx macros. Ensure the Rust type (e.g., `String` or `uuid::Uuid`) strictly matches the PostgreSQL column type. 
*   **Compile-time Verification:** Utilize `sqlx::query!` or `sqlx::query_as!` macros to enforce compile-time checking against our PostgreSQL schema. Avoid implicit type conversions.
*   **Transaction Integrity:** The generation of the new sync token and the recording of the state must occur within a single SQLx transaction (`sqlx::Transaction`) to avoid race conditions.

**Tech Lead Synthesis & Risks:**
*   **Synthesis:** SQLx 0.8.6 correctly forces us to be honest about our database types. We need to align the domain's `SyncToken` wrapper type with the exact Postgres schema definition, implementing `sqlx::Type` if it's a custom domain type.
*   **Risk:** A silent failure means our database error mapping in Axum's `IntoResponse` implementation might be swallowing SQLx errors instead of logging them. I am mandating an audit of our `AppError` implementation to ensure SQLx panics/errors are logged via `tracing::error!` before being masked as generic 500 Internal Server Errors.
