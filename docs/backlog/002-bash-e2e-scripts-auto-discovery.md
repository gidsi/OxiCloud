# PRIORITY 2: Dynamic Auto-Discovery & CI Integration for Bash E2E Scripts

**Persona:** OxiCloud Contributor  
**Business Value:** End-to-End tests simulate real-world clients like Thunderbird and DAVx5. Including bash E2E scripts in our dynamic runner ensures that complex client sync flows are verified on every PR, preventing regressions that directly break the user's workflow.

**User Story:**
> **As an** OxiCloud Contributor,
> **I want to** add a new `*_test.sh` E2E script and have the unified test runner automatically execute it alongside API tests,
> **so that** we catch regressions in desktop and mobile client syncs without relying on developers to manually register new bash scripts in the CI pipeline.

**Acceptance Criteria (Gherkin):**
