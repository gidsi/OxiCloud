# PRIORITY 1: Dynamic Auto-Discovery & CI Integration for Hurl API Tests

**Persona:** OxiCloud Contributor  
**Business Value:** API tests are the bedrock of our CalDAV/CardDAV sync. By automating their discovery, we guarantee that every new API safeguard written by a contributor is actively protecting our users' data, without requiring manual bookkeeping in runner scripts.

**User Story:**
> **As an** OxiCloud Contributor,
> **I want to** drop a new `.hurl` API test file into the repository and have the CI pipeline automatically discover and run it,
> **so that** I can expand our sync API test coverage without having to manually update hardcoded lists in the runner configuration.

**Acceptance Criteria (Gherkin):**
