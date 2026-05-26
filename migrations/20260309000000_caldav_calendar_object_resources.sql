-- ============================================================
-- OxiCloud CalDAV calendar object PUT / GET support
--
-- Adds stable CalDAV resource names and persisted strong ETags
-- for calendar object resources:
--   /caldav/{username}/{calendar-path}/{resource_path}
-- ============================================================

CREATE EXTENSION IF NOT EXISTS pgcrypto;

ALTER TABLE IF EXISTS caldav.calendar_events
    ADD COLUMN IF NOT EXISTS resource_path TEXT;

ALTER TABLE IF EXISTS caldav.calendar_events
    ADD COLUMN IF NOT EXISTS etag VARCHAR(64);

ALTER TABLE IF EXISTS caldav.calendar_events
    ADD COLUMN IF NOT EXISTS ical_data TEXT;

ALTER TABLE IF EXISTS caldav.calendar_events
    ALTER COLUMN resource_path SET DEFAULT (gen_random_uuid()::text || '.ics');

ALTER TABLE IF EXISTS caldav.calendar_events
    ALTER COLUMN etag SET DEFAULT encode(gen_random_bytes(32), 'hex');

ALTER TABLE IF EXISTS caldav.calendar_events
    ALTER COLUMN ical_data SET DEFAULT '';

UPDATE caldav.calendar_events
SET ical_uid = id::text
WHERE btrim(ical_uid) = '';

WITH ranked AS (
    SELECT
        id,
        calendar_id,
        ical_uid,
        row_number() OVER (
            PARTITION BY calendar_id, ical_uid
            ORDER BY updated_at DESC, id
        ) AS rn
    FROM caldav.calendar_events
),
duplicates AS (
    SELECT
        id,
        left(
            COALESCE(
                NULLIF(
                    trim(BOTH '-' FROM regexp_replace(btrim(ical_uid), '[[:cntrl:]/]+', '-', 'g')),
                    ''
                ),
                id::text
            ),
            190
        ) || '-' || id::text AS new_ical_uid
    FROM ranked
    WHERE rn > 1
)
UPDATE caldav.calendar_events AS events
SET
    ical_uid = duplicates.new_ical_uid,
    ical_data =
        'BEGIN:VCALENDAR' || E'\r\n' ||
        'VERSION:2.0' || E'\r\n' ||
        'PRODID:-//OxiCloud//NONSGML v1.0//EN' || E'\r\n' ||
        'BEGIN:VEVENT' || E'\r\n' ||
        'UID:' || duplicates.new_ical_uid || E'\r\n' ||
        'DTSTAMP:' || to_char(events.updated_at AT TIME ZONE 'UTC', 'YYYYMMDD"T"HH24MISS"Z"') || E'\r\n' ||
        'DTSTART:' || to_char(events.start_time AT TIME ZONE 'UTC', 'YYYYMMDD"T"HH24MISS"Z"') || E'\r\n' ||
        'DTEND:' || to_char(events.end_time AT TIME ZONE 'UTC', 'YYYYMMDD"T"HH24MISS"Z"') || E'\r\n' ||
        'SUMMARY:' || replace(replace(events.summary, E'\r', ''), E'\n', E'\\n') || E'\r\n' ||
        CASE
            WHEN events.description IS NOT NULL THEN
                'DESCRIPTION:' || replace(replace(events.description, E'\r', ''), E'\n', E'\\n') || E'\r\n'
            ELSE ''
        END ||
        CASE
            WHEN events.location IS NOT NULL THEN
                'LOCATION:' || replace(replace(events.location, E'\r', ''), E'\n', E'\\n') || E'\r\n'
            ELSE ''
        END ||
        CASE
            WHEN events.rrule IS NOT NULL THEN
                'RRULE:' || replace(replace(events.rrule, E'\r', ''), E'\n', '') || E'\r\n'
            ELSE ''
        END ||
        'END:VEVENT' || E'\r\n' ||
        'END:VCALENDAR'
FROM duplicates
WHERE events.id = duplicates.id;

UPDATE caldav.calendar_events
SET ical_data =
    'BEGIN:VCALENDAR' || E'\r\n' ||
    'VERSION:2.0' || E'\r\n' ||
    'PRODID:-//OxiCloud//NONSGML v1.0//EN' || E'\r\n' ||
    'BEGIN:VEVENT' || E'\r\n' ||
    'UID:' || ical_uid || E'\r\n' ||
    'DTSTAMP:' || to_char(updated_at AT TIME ZONE 'UTC', 'YYYYMMDD"T"HH24MISS"Z"') || E'\r\n' ||
    'DTSTART:' || to_char(start_time AT TIME ZONE 'UTC', 'YYYYMMDD"T"HH24MISS"Z"') || E'\r\n' ||
    'DTEND:' || to_char(end_time AT TIME ZONE 'UTC', 'YYYYMMDD"T"HH24MISS"Z"') || E'\r\n' ||
    'SUMMARY:' || replace(replace(summary, E'\r', ''), E'\n', E'\\n') || E'\r\n' ||
    CASE
        WHEN description IS NOT NULL THEN
            'DESCRIPTION:' || replace(replace(description, E'\r', ''), E'\n', E'\\n') || E'\r\n'
        ELSE ''
    END ||
    CASE
        WHEN location IS NOT NULL THEN
            'LOCATION:' || replace(replace(location, E'\r', ''), E'\n', E'\\n') || E'\r\n'
        ELSE ''
    END ||
    CASE
        WHEN rrule IS NOT NULL THEN
            'RRULE:' || replace(replace(rrule, E'\r', ''), E'\n', '') || E'\r\n'
        ELSE ''
    END ||
    'END:VEVENT' || E'\r\n' ||
    'END:VCALENDAR'
WHERE ical_data IS NULL
   OR btrim(ical_data) = '';

UPDATE caldav.calendar_events
SET etag = btrim(etag)
WHERE etag IS NOT NULL
  AND etag IS DISTINCT FROM btrim(etag);

UPDATE caldav.calendar_events
SET etag = regexp_replace(etag, '^W/"(.+)"$', '\1')
WHERE etag ~ '^W/".+"$';

UPDATE caldav.calendar_events
SET etag = trim(BOTH '"' FROM etag)
WHERE etag LIKE '"%"';

UPDATE caldav.calendar_events
SET etag = encode(digest(ical_data, 'sha256'), 'hex')
WHERE etag IS NULL
   OR btrim(etag) = ''
   OR etag ~ '["[:cntrl:]]'
   OR etag ~* '^W/';

WITH source AS (
    SELECT
        id,
        calendar_id,
        COALESCE(
            NULLIF(btrim(resource_path), ''),
            NULLIF(btrim(ical_uid), ''),
            id::text
        ) AS raw_path
    FROM caldav.calendar_events
),
normalized AS (
    SELECT
        id,
        calendar_id,
        CASE
            WHEN lower(base_segment) LIKE '%.ics' THEN base_segment
            ELSE base_segment || '.ics'
        END AS base_path
    FROM (
        SELECT
            id,
            calendar_id,
            COALESCE(
                NULLIF(
                    trim(BOTH '-' FROM regexp_replace(raw_path, '[[:cntrl:]/]+', '-', 'g')),
                    ''
                ),
                id::text
            ) AS base_segment
        FROM source
    ) AS cleaned
),
deduplicated AS (
    SELECT
        id,
        CASE
            WHEN row_number() OVER (
                PARTITION BY calendar_id, base_path
                ORDER BY id
            ) = 1
            THEN base_path
            ELSE left(regexp_replace(base_path, '\.ics$', '', 'i'), 200) || '-' || id::text || '.ics'
        END AS new_resource_path
    FROM normalized
)
UPDATE caldav.calendar_events AS events
SET resource_path = deduplicated.new_resource_path
FROM deduplicated
WHERE events.id = deduplicated.id
  AND events.resource_path IS DISTINCT FROM deduplicated.new_resource_path;

ALTER TABLE IF EXISTS caldav.calendar_events
    ALTER COLUMN resource_path SET NOT NULL;

ALTER TABLE IF EXISTS caldav.calendar_events
    ALTER COLUMN etag SET NOT NULL;

ALTER TABLE IF EXISTS caldav.calendar_events
    ALTER COLUMN ical_data SET NOT NULL;

DO $$
BEGIN
    IF to_regclass('caldav.calendar_events') IS NOT NULL
       AND NOT EXISTS (
            SELECT 1
            FROM pg_constraint
            WHERE conname = 'chk_calendar_events_resource_path_not_empty'
              AND conrelid = 'caldav.calendar_events'::regclass
       )
    THEN
        ALTER TABLE caldav.calendar_events
            ADD CONSTRAINT chk_calendar_events_resource_path_not_empty
            CHECK (btrim(resource_path) <> '');
    END IF;
END $$;

DO $$
BEGIN
    IF to_regclass('caldav.calendar_events') IS NOT NULL
       AND NOT EXISTS (
            SELECT 1
            FROM pg_constraint
            WHERE conname = 'chk_calendar_events_resource_path_single_segment'
              AND conrelid = 'caldav.calendar_events'::regclass
       )
    THEN
        ALTER TABLE caldav.calendar_events
            ADD CONSTRAINT chk_calendar_events_resource_path_single_segment
            CHECK (resource_path !~ '[[:cntrl:]/]');
    END IF;
END $$;

DO $$
BEGIN
    IF to_regclass('caldav.calendar_events') IS NOT NULL
       AND NOT EXISTS (
            SELECT 1
            FROM pg_constraint
            WHERE conname = 'chk_calendar_events_etag_not_empty'
              AND conrelid = 'caldav.calendar_events'::regclass
       )
    THEN
        ALTER TABLE caldav.calendar_events
            ADD CONSTRAINT chk_calendar_events_etag_not_empty
            CHECK (btrim(etag) <> '');
    END IF;
END $$;

DO $$
BEGIN
    IF to_regclass('caldav.calendar_events') IS NOT NULL
       AND NOT EXISTS (
            SELECT 1
            FROM pg_constraint
            WHERE conname = 'chk_calendar_events_etag_strong_unquoted'
              AND conrelid = 'caldav.calendar_events'::regclass
       )
    THEN
        ALTER TABLE caldav.calendar_events
            ADD CONSTRAINT chk_calendar_events_etag_strong_unquoted
            CHECK (etag !~ '["[:cntrl:]]' AND etag !~* '^W/');
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_calendar_events_calendar_resource_path_unique
    ON caldav.calendar_events(calendar_id, resource_path);

CREATE UNIQUE INDEX IF NOT EXISTS idx_calendar_events_calendar_ical_uid_unique
    ON caldav.calendar_events(calendar_id, ical_uid);

CREATE INDEX IF NOT EXISTS idx_calendar_events_calendar_etag
    ON caldav.calendar_events(calendar_id, etag);

COMMENT ON COLUMN caldav.calendar_events.resource_path IS
    'Stable CalDAV object resource path segment, e.g. event1.ics, used in /caldav/{username}/{calendar-path}/{resource_path}.';

COMMENT ON COLUMN caldav.calendar_events.etag IS
    'Strong unquoted ETag token for CalDAV object resources; HTTP responses wrap this value in quotes.';

COMMENT ON COLUMN caldav.calendar_events.ical_data IS
    'Full stored iCalendar object data for lossless CalDAV round-trip fidelity.';
