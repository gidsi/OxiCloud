-- Dynamic Auto-Discovery & CI Integration for Bash E2E Scripts
--
-- This story updates the xtask/E2E runner behavior and CI execution path only.
-- It does not require any database schema changes.
--
-- Keep this migration intentionally safe and idempotent so sqlx migrate can run it
-- against databases with existing data without modifying application state.

SELECT 1;
