# Priority 3: Native Apple Ecosystem Compatibility (iOS / macOS Strictness)

**Context:** Apple Calendar and Apple Contacts are notoriously strict. They often require absolute URLs in the redirect headers and will fail silently if the `.well-known` redirect doesn't behave exactly as Apple's internal networking stack expects. Delivering this slice allows users to fully drop iCloud.

**User Story:**
**As a** privacy-conscious self-hoster integrated into the Apple ecosystem,
**I want** to add my OxiCloud account natively through iOS/macOS "Add Account" settings using just my server address,
**So that** I can replace iCloud completely without encountering silent failures or broken sync states on my iPhone or Mac.

**Acceptance Criteria:**

**Scenario 1: iOS Calendar native account creation**
* **Given** an operational OxiCloud server
* **When** a user goes to iOS Settings -> Calendar -> Accounts -> Add Account -> Other -> Add CalDAV Account
* **And** enters only `cloud.example.com` in the Server field, alongside their username and password
* **Then** iOS must receive a valid 301 redirect from `/.well-known/caldav`
* **And** iOS must successfully verify the account credentials and show "Calendars" and "Reminders" as syncable options.

**Scenario 2: macOS Contacts native account creation**
* **Given** an operational OxiCloud server
* **When** a user adds a CardDAV account in macOS System Settings using the "Automatic" configuration type and their base domain
* **Then** the `.well-known/carddav` endpoint must provide a 301 redirect formatted in a way that macOS accepts (supporting PROPFIND requests immediately following the GET redirect)
* **And** the macOS Contacts app must successfully pull down the user's OxiCloud address book.

**Security Constraints (Security Reviewer):**
* **Host Header Injection:** When generating absolute URLs to satisfy Apple clients, we MUST NOT reflect the incoming `Host` header. Attackers can forge this header, causing the server to respond with a malicious absolute URL.

**Technical Constraints (Codebase/Rust Expert):**
* **Absolute URLs via Axum State:** Since Apple networking often drops relative redirects (like `/dav/`), we must construct an absolute URL.
* Inject our application configuration state using `axum::extract::State<Arc<AppConfig>>`.
* The `AppConfig` struct must contain the canonical server base URL (e.g., loaded from the environment file during server startup).
* **Format implementation:** Return `Redirect::permanent(&format!("{}/dav/", state.config.base_url))`.

**Tech Lead Synthesis & Risks:**
* **Risk (Misconfiguration):** If the server admin incorrectly sets `AppConfig.base_url` (e.g., using a local IP or `http://` instead of the public `https://` domain), standard clients might work (if we used relative redirects), but Apple clients will immediately break due to the strict absolute URL generation.
* **Architectural Coherence:** Relying on `State<Arc<AppConfig>>` perfectly aligns with our existing, idiomatic Axum state management patterns.
* **Action Item:** I will enforce a startup check in `main.rs` that validates `AppConfig.base_url` starts with `https://` (unless a specific `--dev` flag is passed) to catch self-hoster misconfigurations early and prevent silent iOS discovery failures. Additionally, we will write an Axum route test using `axum::test_helpers` to ensure absolute URLs are reliably generated in the `Location` header.
