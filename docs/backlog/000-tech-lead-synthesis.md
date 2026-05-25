# Tech Lead Synthesis & Risk Assessment

As the Tech Lead, reviewing the combination of dynamic auto-discovery across both Hurl and Bash endpoints brings up a few structural risks that we must mitigate to protect our development velocity and system integrity:

1.  **Risk: Database State Collision (The "Flaky Test" Anti-Pattern)**
    *   *Context:* Dynamically discovering and running multiple `.hurl` and `*_test.sh` scripts means our test count will grow rapidly. If these run sequentially or concurrently against the same local SQLx PostgreSQL database instance, one test modifying a CalDAV event will cause another test asserting on that event to fail.
    *   *Mitigation:* We must enforce a **"Shared Nothing" test architecture**. The unified runner needs to either (A) spin up a new Postgres schema per test execution, or (B) mandate that every script dynamically generates unique UUIDs for users/resources so they do not overlap.
2.  **Risk: Indefinitely Hanging CI Runs**
    *   *Context:* E2E Bash scripts executing `curl` commands or client binaries can hang indefinitely if the underlying Axum server deadlocks or drops a connection.
    *   *Mitigation:* The unified runner MUST wrap test executions with a strict timeout (e.g., `timeout 30s bash ...` or via `tokio::time::timeout` in Rust). If a test exceeds its time boundary, it is killed and marked as a failure.
3.  **Risk: Maintenance of a Monolithic Bash Runner**
    *   *Context:* Writing complex globbing, aggregation, and timeout logic in a pure `bash` script usually devolves into an unmaintainable mess.
    *   *Mitigation:* **Adopt the `cargo xtask` pattern.** Let's leverage our team's existing Rust expertise. A small Rust application living in `xtask/` can use the `glob` and `tokio` crates to discover files, manage the PostgreSQL lifecycle via `sqlx-cli`, parallelize test execution safely, handle timeouts, and provide beautiful, unified error aggregation for both Hurl and Bash. This aligns tightly with our existing architectural competencies.
