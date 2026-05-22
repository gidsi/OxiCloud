# Priority 2: Seamless Address Book Setup for Mobile (CardDAV via DAVx5)

**Context:** Android users rely heavily on DAVx5 to replace Google Contacts. DAVx5 uses `.well-known/carddav` to automatically configure the account. Without this, mobile onboarding is highly frustrating.

**User Story:**
**As a** privacy-conscious self-hoster on Android,
**I want** to connect the DAVx5 app using only my base server domain and login credentials,
**So that** I can immediately back up and sync my phone's address book without manually configuring URL subdirectories.

**Acceptance Criteria:**

**Scenario 1: Standard CardDAV discovery redirect**
* **Given** an operational OxiCloud server hosted at `https://cloud.example.com`
* **When** the DAVx5 client makes an HTTP request to `https://cloud.example.com/.well-known/carddav`
* **Then** the server must respond with an HTTP redirect
* **And** the `Location` header must point to the root CardDAV endpoint of the server
* **And** DAVx5 must successfully populate the user's address books upon following the redirect.

**Scenario 2: Preserving protocol scheme**
* **Given** OxiCloud is accessed via a reverse proxy using HTTPS
* **When** the client queries `/.well-known/carddav`
* **Then** the resulting redirect `Location` URL must preserve the `https://` scheme or use a valid relative path (`/dav/`) so the mobile client does not downgrade to insecure HTTP.

**Security Constraints (Security Reviewer):**
* **Proxy Spoofing Prevention:** Avoid directly trusting `X-Forwarded-Proto` or `X-Forwarded-Host` HTTP headers to build the redirect URL unless Axum is explicitly configured with a trusted proxies layer. Untrusted headers can lead to malicious scheme downgrades or host spoofing.

**Technical Constraints (Codebase/Rust Expert):**
* **Axum Handlers:** Add `async fn carddav_discovery() -> impl IntoResponse` to the `well_known_router()`.
* **Database (SQLx):** No SQLx dependencies are needed here; keep the handler fully stateless to maximize performance.

**Tech Lead Synthesis & Risks:**
* **Risk (HTTPS Downgrade):** Mobile sync clients will aggressively refuse connections or silently fail if redirected to an HTTP endpoint instead of HTTPS. 
* **Mitigation:** The safest way to preserve the protocol scheme across any unpredictable reverse proxy setup (Nginx, Traefik, Caddy) is to use an absolute path based on a strictly configured server variable rather than relying on HTTP request headers.
