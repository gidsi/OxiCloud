# Story 3: Secure Infrastructure API with Healthcheck Bypass

**Priority:** Medium

**Technical Context:** 
The epic to enforce strict authentication caused failing tests because our reverse proxies (Traefik/Nginx) are getting 401s on the `/health` endpoint. We are keeping the security enforcement and the bypass exception in exactly the same vertical slice to prevent test deadlocks in our application layer.

**User Story:**
As an OxiCloud Sysadmin, 
I want to secure the CalDAV/CardDAV APIs with strict authentication while explicitly bypassing the `/health` endpoint, 
so that my personal data is kept private but my reverse proxy can still successfully monitor server uptime.

**Acceptance Criteria:**
*   **Given** the OxiCloud server is running and configured with Axum routing
*   **When** an unauthenticated client sends a `GET`, `PROPFIND`, or `OPTIONS` request to any `/dav/*` endpoint
*   **Then** the server must reject the request with a `401 Unauthorized` and a `WWW-Authenticate` header
*   **When** an unauthenticated monitoring service sends a `GET` request to `/health`
*   **Then** the server must explicitly bypass the auth middleware and return a `200 OK` with the system status.

**Security Constraints (Security Reviewer):**
*   **Healthcheck Data Minimization:** The `/health` endpoint must only return generic status information (e.g., `{"status": "ok"}`). It must not leak internal database states, software versions, or cluster topologies.
*   **Route Bleeding Prevention:** Ensure bypass rules do not allow path traversal (e.g., `/health/../dav/`) to evade authentication.

**Architectural Constraints (Codebase/Rust Expert):**
*   **Axum Router Hierarchy:** Do *not* implement conditional path checking inside a global middleware. Instead, utilize Axum's native routing hierarchy. Define a public router (for `/health`) and a protected router (for `/dav/*`). Apply the auth `axum::middleware::from_extractor` (or `from_fn`) *only* to the protected router using `.route_layer()`.
*   **Combine Routers Safely:** Merge the nested routers using `Router::merge` at the top level of the application state.

**Tech Lead Synthesis & Risks:**
*   **Synthesis:** Trying to be clever with middleware path inspections always leads to brittle security and failing tests. We will solve this structurally. Axum provides beautiful composability for routing—we will split our routes into `public_router` and `dav_router`, attaching the auth layer strictly to the latter.
*   **Risk:** We must ensure that the `OPTIONS` preflight requests on the protected routes are handled correctly by CORS middleware *before* the auth layer, otherwise they will be blocked by the 401 response, breaking web-based CalDAV clients.
