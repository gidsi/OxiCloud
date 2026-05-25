-- ============================================================
-- OxiCloud CalDAV MKCALENDAR support
-- Adds a stable DAV collection path/slug for calendars.
--
-- Required for canonical calendar URIs:
--   /caldav/{username}/{calendar-path}/
--
-- The DAV path is intentionally distinct from the display name.
-- Duplicate detection for MKCALENDAR must be based on:
--   (owner_id, path)
-- ============================================================

CREATE EXTENSION IF NOT EXISTS pgcrypto;

ALTER TABLE IF EXISTS caldav.calendars
    ADD COLUMN IF NOT EXISTS path TEXT;

ALTER TABLE IF EXISTS caldav.calendars
    ALTER COLUMN path SET DEFAULT ('calendar-' || gen_random_uuid()::text);

WITH source AS (
    SELECT
        id,
        owner_id,
        COALESCE(
            NULLIF(btrim(path), ''),
            NULLIF(
                trim(BOTH '-' FROM regexp_replace(lower(btrim(name)), '[^a-z0-9._~-]+', '-', 'g')),
                ''
            ),
            'calendar'
        ) AS raw_path
    FROM caldav.calendars
),
normalized AS (
    SELECT
        id,
        owner_id,
        COALESCE(
            NULLIF(
                trim(BOTH '-' FROM regexp_replace(raw_path, '/+', '-', 'g')),
                ''
            ),
            'calendar'
        ) AS base_path
    FROM source
),
deduplicated AS (
    SELECT
        id,
        CASE
            WHEN row_number() OVER (
                PARTITION BY owner_id, base_path
                ORDER BY id
            ) = 1
            THEN base_path
            ELSE left(base_path, 218) || '-' || id::text
        END AS new_path
    FROM normalized
)
UPDATE caldav.calendars AS calendars
SET path = deduplicated.new_path
FROM deduplicated
WHERE calendars.id = deduplicated.id
  AND calendars.path IS DISTINCT FROM deduplicated.new_path;

ALTER TABLE IF EXISTS caldav.calendars
    ALTER COLUMN path SET NOT NULL;

DO $$
BEGIN
    IF to_regclass('caldav.calendars') IS NOT NULL
       AND NOT EXISTS (
            SELECT 1
            FROM pg_constraint
            WHERE conname = 'chk_calendars_path_not_empty'
              AND conrelid = 'caldav.calendars'::regclass
       )
    THEN
        ALTER TABLE caldav.calendars
            ADD CONSTRAINT chk_calendars_path_not_empty
            CHECK (btrim(path) <> '');
    END IF;
END $$;

DO $$
BEGIN
    IF to_regclass('caldav.calendars') IS NOT NULL
       AND NOT EXISTS (
            SELECT 1
            FROM pg_constraint
            WHERE conname = 'chk_calendars_path_single_segment'
              AND conrelid = 'caldav.calendars'::regclass
       )
    THEN
        ALTER TABLE caldav.calendars
            ADD CONSTRAINT chk_calendars_path_single_segment
            CHECK (position('/' IN path) = 0);
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_calendars_owner_path_unique
    ON caldav.calendars(owner_id, path);

CREATE INDEX IF NOT EXISTS idx_calendars_path
    ON caldav.calendars(path);

COMMENT ON COLUMN caldav.calendars.path IS
    'Stable CalDAV collection path/slug used in /caldav/{username}/{path}/; distinct from display name.';
