-- ════════════════════════════════════════════════════════════════════════════
-- Prelude for magic-link external authentication
-- ════════════════════════════════════════════════════════════════════════════
-- This migration is purely additive — it lands the schema bits needed by the
-- subsequent magic-link work without altering existing rows or behaviour:
--
--   * `given_name` / `family_name` — optional human-readable identity fields.
--     Populated from OIDC standard claims (given_name, family_name) at JIT
--     provisioning. External users start with both NULL; either side can be
--     filled in later via a profile-edit endpoint.
--
-- Note on username length: `auth.users.username` is already `TEXT` with no
-- DB-level length constraint, so it can already hold the 254-char RFC 5321
-- maximum required for email-as-username. The widening happens at the
-- entity-level validator (`User::validate_username`), not the schema.

ALTER TABLE auth.users
    ADD COLUMN IF NOT EXISTS given_name  TEXT NULL,
    ADD COLUMN IF NOT EXISTS family_name TEXT NULL;

COMMENT ON COLUMN auth.users.given_name IS
    'Optional first/given name. Populated from OIDC standard claim `given_name` at JIT provisioning; settable via profile-edit endpoint. NULL until explicitly set.';

COMMENT ON COLUMN auth.users.family_name IS
    'Optional last/family name. Populated from OIDC standard claim `family_name` at JIT provisioning; settable via profile-edit endpoint. NULL until explicitly set.';
