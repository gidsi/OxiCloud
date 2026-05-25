-- DAV authentication hardening support.
--
-- This migration is intentionally conservative:
--   1. Adds a durable audit table for failed DAV Basic Auth attempts so
--      app-layer audit events can be persisted and queried by operators.
--   2. Adds lookup indexes used by Basic Auth verification paths to reduce
--      database work for CalDAV/CardDAV clients that authenticate frequently.
--
-- All DDL is guarded for safe re-runs by sqlx migrate.

CREATE SCHEMA IF NOT EXISTS auth;

CREATE TABLE IF NOT EXISTS auth.dav_auth_failures (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid()
);

ALTER TABLE auth.dav_auth_failures
    ADD COLUMN IF NOT EXISTS occurred_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP;

ALTER TABLE auth.dav_auth_failures
    ADD COLUMN IF NOT EXISTS client_ip TEXT NOT NULL DEFAULT '';

ALTER TABLE auth.dav_auth_failures
    ADD COLUMN IF NOT EXISTS username TEXT NOT NULL DEFAULT '';

ALTER TABLE auth.dav_auth_failures
    ADD COLUMN IF NOT EXISTS method TEXT NOT NULL DEFAULT '';

ALTER TABLE auth.dav_auth_failures
    ADD COLUMN IF NOT EXISTS path TEXT NOT NULL DEFAULT '';

ALTER TABLE auth.dav_auth_failures
    ADD COLUMN IF NOT EXISTS user_agent TEXT NOT NULL DEFAULT '';

ALTER TABLE auth.dav_auth_failures
    ADD COLUMN IF NOT EXISTS reason TEXT NOT NULL DEFAULT 'invalid_credentials';

ALTER TABLE auth.dav_auth_failures
    ADD COLUMN IF NOT EXISTS auth_scheme TEXT NOT NULL DEFAULT 'Basic';

ALTER TABLE auth.dav_auth_failures
    ADD COLUMN IF NOT EXISTS protocol TEXT NOT NULL DEFAULT 'DAV';

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_dav_auth_failures_auth_scheme_not_empty'
          AND conrelid = 'auth.dav_auth_failures'::regclass
    ) THEN
        ALTER TABLE auth.dav_auth_failures
            ADD CONSTRAINT chk_dav_auth_failures_auth_scheme_not_empty
            CHECK (char_length(auth_scheme) > 0);
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_dav_auth_failures_protocol_not_empty'
          AND conrelid = 'auth.dav_auth_failures'::regclass
    ) THEN
        ALTER TABLE auth.dav_auth_failures
            ADD CONSTRAINT chk_dav_auth_failures_protocol_not_empty
            CHECK (char_length(protocol) > 0);
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_dav_auth_failures_reason_not_empty'
          AND conrelid = 'auth.dav_auth_failures'::regclass
    ) THEN
        ALTER TABLE auth.dav_auth_failures
            ADD CONSTRAINT chk_dav_auth_failures_reason_not_empty
            CHECK (char_length(reason) > 0);
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_dav_auth_failures_occurred_at
    ON auth.dav_auth_failures(occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_dav_auth_failures_client_ip_occurred_at
    ON auth.dav_auth_failures(client_ip, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_dav_auth_failures_username_occurred_at
    ON auth.dav_auth_failures(username, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_dav_auth_failures_path_occurred_at
    ON auth.dav_auth_failures(path, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_dav_auth_failures_reason_occurred_at
    ON auth.dav_auth_failures(reason, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_app_passwords_basic_auth_lookup
    ON auth.app_passwords(user_id, prefix, created_at DESC)
    WHERE active = TRUE;

CREATE INDEX IF NOT EXISTS idx_app_passwords_user_active_expires
    ON auth.app_passwords(user_id, active, expires_at);

CREATE INDEX IF NOT EXISTS idx_app_passwords_cleanup
    ON auth.app_passwords(active, expires_at);

COMMENT ON TABLE auth.dav_auth_failures IS
    'Audit log of failed DAV Basic Authentication attempts for CalDAV/CardDAV/WebDAV clients';

COMMENT ON COLUMN auth.dav_auth_failures.occurred_at IS
    'Timestamp when the failed authentication attempt occurred';

COMMENT ON COLUMN auth.dav_auth_failures.client_ip IS
    'Best-effort client IP address extracted by the application layer';

COMMENT ON COLUMN auth.dav_auth_failures.username IS
    'Username presented in the Basic Auth credentials, if available';

COMMENT ON COLUMN auth.dav_auth_failures.method IS
    'HTTP method used for the failed DAV request, for example OPTIONS or PROPFIND';

COMMENT ON COLUMN auth.dav_auth_failures.path IS
    'Requested DAV path that triggered the authentication failure';

COMMENT ON COLUMN auth.dav_auth_failures.user_agent IS
    'User-Agent header value, if supplied by the client';

COMMENT ON COLUMN auth.dav_auth_failures.reason IS
    'Machine-readable failure reason such as missing_credentials, malformed_credentials, invalid_credentials, expired_credentials, revoked_credentials, or app_passwords_disabled';

COMMENT ON COLUMN auth.dav_auth_failures.auth_scheme IS
    'Authentication scheme involved in the failure; DAV clients are expected to use Basic';

COMMENT ON COLUMN auth.dav_auth_failures.protocol IS
    'Protocol namespace for the request, normally DAV';
