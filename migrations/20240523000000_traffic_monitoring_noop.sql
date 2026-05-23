-- Traffic Monitoring (HTTP Metrics with Cardinality Protection)
--
-- This feature exposes Prometheus metrics from the application process using
-- Axum middleware and metrics / metrics-exporter-prometheus.
--
-- No database-backed metrics storage is required or desired:
--   - HTTP metrics must be aggregated in-process/exported to Prometheus.
--   - Raw request paths must never be persisted.
--   - Route labels must come from Axum MatchedPath or the static UNMATCHED label.
--
-- Therefore this migration intentionally performs no schema changes.

BEGIN;

-- No-op migration.
-- Kept as an explicit migration file so sqlx migrate has a concrete migration
-- for the Traffic Monitoring story without introducing unnecessary tables,
-- columns, or indexes.

COMMIT;
