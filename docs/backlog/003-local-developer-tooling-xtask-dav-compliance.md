# 3. Priority: Medium
**Title:** Local Developer tooling (xtask) for DAV Compliance Testing

**Business Value:** 
Once developers can see their compliance score in CI, they need a fast feedback loop to fix failing RFC tests locally. Waiting 10 minutes for a GitHub Action to report a score is a waste of engineering time.

**User Story:**
As an OxiCloud developer, I want to execute the DAV compliance suite locally via our build tooling (e.g., `cargo xtask test-dav`) so that I can iteratively fix sync bugs and verify my compliance score before pushing code to CI.

**Acceptance Criteria:**
* **Given** I have the OxiCloud repository cloned locally,
* **When** I execute the local DAV compliance command,
* **Then** the tooling must automatically spin up a local ephemeral instance, execute the compliance suite against it, and tear the instance down.
* **Given** the local compliance suite finishes executing,
* **When** the results are output to the terminal,
* **Then** I must see the exact same human-readable score breakdown (pass/fail percentage) that CI generates,
* **And** the terminal output must list the specific RFC tests that failed to guide my debugging.

**Security Constraints (Security Reviewer):**
* Ensure the `xtask` tooling explicitly ignores local `.env` files that might point to a real or persistent local development database. We cannot risk a destructive test setup wiping real local engineering data.
* Hardcode temporary, randomized secure credentials solely for the test setup execution.

**Architectural Constraints (Codebase/Rust Expert):**
* Use `testcontainers-rs` to programmatically provision Postgres within the `xtask` runner. Avoid forcing the developer to interact with standard `docker-compose.yml` configs.
* The Axum server must be executed in a Tokio spawned background task (`tokio::spawn`).
* Implement graceful shutdown mechanisms using a Tokio `CancellationToken` mapped to OS signals. The SQLx connection pool (`PgPool`) MUST be explicitly closed (`pool.close().await`) before tearing down the `testcontainers` Postgres instance to prevent zombie connections and ensure clean exit codes.

**Tech Lead Synthesis & Risks:**
* **Risk:** Developers without Docker running (or on unsupported setups like rootless Podman) will encounter opaque runtime panics because `testcontainers-rs` cannot connect to the daemon.
* **Mitigation:** We must embed an early pre-flight check in `xtask test-dav` that verifies the Docker daemon is accessible. If not, it must fail fast with a highly actionable error message. Relying exclusively on Rust via `xtask` for this will guarantee cross-platform consistency (Windows/macOS/Linux) and guard us against the entropy of brittle shell scripts.
