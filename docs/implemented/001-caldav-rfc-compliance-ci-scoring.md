# 1. Priority: Highest
**Title:** CalDAV RFC Compliance CI Scoring (Non-Blocking)

**Business Value:** 
Apple Calendar and Thunderbird users are our most vocal complainers regarding sync conflicts. We need immediate visibility on how our daily code changes impact calendar syncing without grinding the team to a halt.

**User Story:**
As an OxiCloud contributor, I want to see an automated, non-blocking CalDAV compliance score on my Pull Requests so that I know if my code improves or degrades Apple Calendar/Thunderbird event syncing without my PR being blocked by existing RFC failures.

**Acceptance Criteria:**
* **Given** a Pull Request is opened or updated,
* **When** the CI pipeline triggers the chosen CalDAV compliance suite (e.g., CalDAVTester) against an ephemeral OxiCloud instance,
* **Then** the CI step MUST catch any protocol test failures and return `exit 0` (success) so the pipeline continues uninterrupted.
* **Given** the CalDAV compliance suite has finished executing with or without errors,
* **When** the CI step finalises,
* **Then** it must parse the raw output to calculate a human-readable pass/fail percentage (Compliance Score),
* **And** automatically post or update a summary comment on the Pull Request displaying this score.

**Security Constraints (Security Reviewer):**
* Ensure the ephemeral OxiCloud instance in CI strictly uses dummy credentials and deterministic, non-sensitive seed data.
* CI output parsing MUST NOT leak any server environment variables or raw internal logs into the GitHub PR comment, to avoid exposing potential vulnerabilities or sensitive stack traces.
* The PR comment integration must strictly use least-privilege GitHub Action tokens (e.g., `pull-requests: write` only).

**Architectural Constraints (Codebase/Rust Expert):**
* Avoid raw bash scripts for orchestrating the test parsing. Build a small Rust parser module within the `xtask` workspace to read the XML/log output from the CalDAV suite and format the Markdown. 
* Rely on `sqlx` to execute migrations on a fresh ephemeral Postgres instance (e.g., GitHub Actions service container) before binding the Axum server.
* Ensure the Axum server's application state (e.g., `State<Arc<AppState>>`) is initialized identically to production but pointed at the ephemeral DB.

**Tech Lead Synthesis & Risks:**
* **Risk:** CI execution time could balloon if pulling and configuring CalDAVTester (which is Python-based) takes too long on every PR.
* **Mitigation:** We should containerize the CalDAV compliance suite or heavily cache the Python environment in CI. We will rely entirely on our Rust `xtask` for the orchestration logic to keep the CI YAML "dumb" and easily reproducible locally.
