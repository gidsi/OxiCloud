# 2. Priority: High
**Title:** CardDAV RFC Compliance CI Scoring (Non-Blocking)

**Business Value:** 
Contact syncing (DAVx5, GNOME Contacts) is our second most critical native integration. Now that the CI scoring pipeline exists for calendars, we must extend it to track our progress towards flawless contact syncing.

**User Story:**
As an OxiCloud contributor, I want to see an automated, non-blocking CardDAV compliance score on my Pull Requests so that I can track our progress towards flawless contact syncing for DAVx5 and Apple Contacts without failing the main branch.

**Acceptance Criteria:**
* **Given** the compliance testing pipeline is established,
* **When** the CardDAV test suite executes in CI and encounters protocol failures,
* **Then** the job MUST NOT fail the overall pipeline (`exit 0`),
* **And** it must parse the CardDAV results into a distinct human-readable score.
* **Given** both CalDAV and CardDAV scores are generated during the CI run,
* **When** the pipeline updates the PR summary comment,
* **Then** it MUST clearly display both the CalDAV and CardDAV compliance percentages as separate, independent metrics.

**Security Constraints (Security Reviewer):**
* CardDAV tests mandate contact data generation. Ensure all vCard data used for testing contains strictly fictitious PII (e.g., no real names, phone numbers, or addresses of actual users or team members).
* Ensure strict separation of test suite user accounts; auth rules and basic auth endpoints must be thoroughly isolated per test runner.

**Architectural Constraints (Codebase/Rust Expert):**
* Extend the `xtask` reporting parser to seamlessly accommodate CardDAV results alongside CalDAV. 
* Ensure the Axum router handles `/.well-known/carddav` autodiscovery routing properly so the test suite can navigate the endpoints without hardcoded paths.
* SQLx connection pooling limits must be evaluated to ensure they gracefully handle the load of dual test suites without pool exhaustion (`max_connections` configuration).

**Tech Lead Synthesis & Risks:**
* **Risk:** Running both suites sequentially might double the CI testing phase duration, worsening developer experience.
* **Mitigation:** We should run the CalDAV and CardDAV suites concurrently in parallel GitHub Action jobs. The `xtask` PR commenter tool must be designed to fetch artifacts and merge these parallel results into a single cohesive PR comment (using a unique comment identifier to update rather than constantly appending new comments).
