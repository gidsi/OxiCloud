-- Resource Planning (Memory Usage Metrics)
--
-- This feature exposes process memory usage through an in-memory Prometheus
-- /metrics endpoint. Metrics are collected by a background task and rendered
-- from process-local state.
--
-- No database schema changes are required:
--   - No metrics persistence table is needed.
--   - No existing tables require new columns.
--   - No data backfill is required.
--   - No indexes are required.

BEGIN;

-- Intentionally no-op migration.
-- Kept as a migration marker so sqlx migrate can record that this story's
-- database impact was reviewed and requires no schema changes.

COMMIT;
