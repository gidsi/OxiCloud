-- Apple .well-known CalDAV/CardDAV compatibility
--
-- This migration is intentionally a no-op.
--
-- The associated application change is limited to Axum handler behavior for:
--   GET  /.well-known/caldav
--   HEAD /.well-known/caldav
--   GET  /.well-known/carddav
--   HEAD /.well-known/carddav
--
-- These endpoints generate 301 absolute redirects from AppState.config.base_url.
-- They do not read from or write to PostgreSQL, and AppConfig.base_url is loaded
-- from application configuration rather than persisted in the database.
--
-- No tables, columns, indexes, constraints, type changes, or data backfills are
-- required for this story.

DO $$
BEGIN
    NULL;
END
$$;
