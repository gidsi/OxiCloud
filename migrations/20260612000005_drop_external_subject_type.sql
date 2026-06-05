-- ════════════════════════════════════════════════════════════════════════════
-- Drop `external` from `storage.access_grants.subject_type` CHECK
-- ════════════════════════════════════════════════════════════════════════════
-- The federated-identity case is now folded into `Subject::User(uuid)` with
-- `auth.users.is_external = TRUE` (PR 2 of the external-users work). Nothing
-- in code ever minted a grant row with subject_type='external', so there
-- are no live rows to migrate — this migration just tightens the CHECK
-- so a stale piece of code can't accidentally start producing them.
--
-- The original CHECK in `20260520000000_rebac_access_grants.sql` was an
-- inline anonymous constraint, so we discover its auto-generated name via
-- pg_constraint before dropping it.

DO $BODY$
DECLARE
    cname TEXT;
BEGIN
    SELECT c.conname INTO cname
    FROM pg_constraint c
    JOIN pg_namespace n ON n.oid = c.connamespace
    JOIN pg_class    t ON t.oid = c.conrelid
    WHERE n.nspname = 'storage'
      AND t.relname = 'access_grants'
      AND c.contype = 'c'
      AND pg_get_constraintdef(c.oid) ILIKE '%subject_type%external%';

    IF cname IS NOT NULL THEN
        EXECUTE 'ALTER TABLE storage.access_grants DROP CONSTRAINT '
                || quote_ident(cname);
    END IF;
END $BODY$;

ALTER TABLE storage.access_grants
    ADD CONSTRAINT access_grants_subject_type_check
    CHECK (subject_type IN ('user', 'group', 'token'));
