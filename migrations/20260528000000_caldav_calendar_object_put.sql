CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE SCHEMA IF NOT EXISTS caldav;

ALTER TABLE IF EXISTS caldav.calendar_events
    ADD COLUMN IF NOT EXISTS resource_name TEXT;

ALTER TABLE IF EXISTS caldav.calendar_events
    ALTER COLUMN resource_name SET DEFAULT ('event-' || gen_random_uuid()::text || '.ics');

ALTER TABLE IF EXISTS caldav.calendar_events
    ADD COLUMN IF NOT EXISTS etag VARCHAR(128);

DO $$
BEGIN
    IF to_regclass('caldav.calendar_events') IS NOT NULL
       AND EXISTS (
           SELECT 1
           FROM information_schema.columns
           WHERE table_schema = 'caldav'
             AND table_name = 'calendar_events'
             AND column_name = 'etag'
       )
    THEN
        ALTER TABLE caldav.calendar_events
            ALTER COLUMN etag TYPE VARCHAR(128);
    END IF;
END $$;

ALTER TABLE IF EXISTS caldav.calendar_events
    ALTER COLUMN etag SET DEFAULT md5(gen_random_uuid()::text);

DO $$
BEGIN
    IF to_regclass('caldav.calendar_events') IS NOT NULL THEN
        WITH prepared AS (
            SELECT
                id,
                (
                    COALESCE(
                        NULLIF(
                            btrim(
                                regexp_replace(
                                    COALESCE(NULLIF(ical_uid, ''), id::text),
                                    '[^A-Za-z0-9._~-]+',
                                    '-',
                                    'g'
                                ),
                                '.-'
                            ),
                            ''
                        ),
                        'event-' || substring(id::text, 1, 8)
                    ) || '.ics'
                ) AS generated_resource_name
            FROM caldav.calendar_events
            WHERE resource_name IS NULL
               OR btrim(resource_name) = ''
        )
        UPDATE caldav.calendar_events e
        SET resource_name = prepared.generated_resource_name
        FROM prepared
        WHERE e.id = prepared.id;

        UPDATE caldav.calendar_events
        SET resource_name = COALESCE(
            NULLIF(
                btrim(
                    regexp_replace(
                        resource_name,
                        '[^A-Za-z0-9._~-]+',
                        '-',
                        'g'
                    ),
                    '.-'
                ),
                ''
            ),
            'event-' || substring(id::text, 1, 8)
        )
        WHERE resource_name IS NULL
           OR resource_name <> COALESCE(
                NULLIF(
                    btrim(
                        regexp_replace(
                            resource_name,
                            '[^A-Za-z0-9._~-]+',
                            '-',
                            'g'
                        ),
                        '.-'
                    ),
                    ''
                ),
                'event-' || substring(id::text, 1, 8)
           );

        UPDATE caldav.calendar_events
        SET resource_name = resource_name || '.ics'
        WHERE lower(resource_name) NOT LIKE '%.ics';

        WITH ranked AS (
            SELECT
                id,
                resource_name,
                row_number() OVER (
                    PARTITION BY calendar_id, resource_name
                    ORDER BY created_at, id
                ) AS rn
            FROM caldav.calendar_events
        )
        UPDATE caldav.calendar_events e
        SET resource_name =
            CASE
                WHEN lower(ranked.resource_name) LIKE '%.ics' THEN
                    left(ranked.resource_name, length(ranked.resource_name) - 4)
                    || '-' || ranked.rn::text || '.ics'
                ELSE
                    ranked.resource_name || '-' || ranked.rn::text
            END
        FROM ranked
        WHERE e.id = ranked.id
          AND ranked.rn > 1;

        UPDATE caldav.calendar_events
        SET etag = md5(
            COALESCE(ical_data, '')
            || ':' || id::text
            || ':' || COALESCE(extract(epoch FROM updated_at)::text, '')
        )
        WHERE etag IS NULL
           OR btrim(etag) = '';

        UPDATE caldav.calendar_events
        SET ical_data =
            'BEGIN:VCALENDAR' || E'\r\n' ||
            'VERSION:2.0' || E'\r\n' ||
            'PRODID:-//OxiCloud//CalDAV//EN' || E'\r\n' ||
            'BEGIN:VEVENT' || E'\r\n' ||
            'UID:' || ical_uid || E'\r\n' ||
            'DTSTAMP:' || to_char(updated_at AT TIME ZONE 'UTC', 'YYYYMMDD"T"HH24MISS"Z"') || E'\r\n' ||
            'DTSTART:' || to_char(start_time AT TIME ZONE 'UTC', 'YYYYMMDD"T"HH24MISS"Z"') || E'\r\n' ||
            'DTEND:' || to_char(end_time AT TIME ZONE 'UTC', 'YYYYMMDD"T"HH24MISS"Z"') || E'\r\n' ||
            'SUMMARY:' || replace(replace(summary, E'\r', ''), E'\n', '\n') || E'\r\n' ||
            'END:VEVENT' || E'\r\n' ||
            'END:VCALENDAR' || E'\r\n'
        WHERE ical_data IS NULL
           OR btrim(ical_data) = '';
    END IF;
END $$;

ALTER TABLE IF EXISTS caldav.calendar_events
    ALTER COLUMN resource_name SET NOT NULL;

ALTER TABLE IF EXISTS caldav.calendar_events
    ALTER COLUMN etag SET NOT NULL;

ALTER TABLE IF EXISTS caldav.calendar_events
    ALTER COLUMN ical_data SET DEFAULT '';

ALTER TABLE IF EXISTS caldav.calendar_events
    ALTER COLUMN ical_data SET NOT NULL;

DO $$
BEGIN
    IF to_regclass('caldav.calendar_events') IS NOT NULL
       AND NOT EXISTS (
           SELECT 1
           FROM pg_constraint
           WHERE conname = 'chk_calendar_events_resource_name_valid'
             AND conrelid = 'caldav.calendar_events'::regclass
       )
    THEN
        ALTER TABLE caldav.calendar_events
            ADD CONSTRAINT chk_calendar_events_resource_name_valid
            CHECK (
                char_length(resource_name) > 0
                AND position('/' IN resource_name) = 0
                AND position(E'\\' IN resource_name) = 0
                AND resource_name <> '.'
                AND resource_name <> '..'
                AND lower(resource_name) LIKE '%.ics'
            )
            NOT VALID;
    END IF;

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
            CHECK (char_length(btrim(etag)) > 0)
            NOT VALID;
    END IF;

    IF to_regclass('caldav.calendar_events') IS NOT NULL
       AND NOT EXISTS (
           SELECT 1
           FROM pg_constraint
           WHERE conname = 'chk_calendar_events_ical_data_not_empty'
             AND conrelid = 'caldav.calendar_events'::regclass
       )
    THEN
        ALTER TABLE caldav.calendar_events
            ADD CONSTRAINT chk_calendar_events_ical_data_not_empty
            CHECK (char_length(btrim(ical_data)) > 0)
            NOT VALID;
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_calendar_events_calendar_resource_unique
    ON caldav.calendar_events(calendar_id, resource_name);

CREATE INDEX IF NOT EXISTS idx_calendar_events_calendar_resource_etag
    ON caldav.calendar_events(calendar_id, resource_name, etag);

CREATE INDEX IF NOT EXISTS idx_calendar_events_calendar_uid
    ON caldav.calendar_events(calendar_id, ical_uid);

CREATE INDEX IF NOT EXISTS idx_calendar_events_calendar_updated_at
    ON caldav.calendar_events(calendar_id, updated_at DESC);

DO $$
BEGIN
    IF to_regclass('caldav.calendar_events') IS NOT NULL
       AND NOT EXISTS (
           SELECT 1
           FROM pg_indexes
           WHERE schemaname = 'caldav'
             AND indexname = 'idx_calendar_events_calendar_uid_unique'
       )
       AND NOT EXISTS (
           SELECT 1
           FROM caldav.calendar_events
           GROUP BY calendar_id, ical_uid
           HAVING COUNT(*) > 1
       )
    THEN
        CREATE UNIQUE INDEX idx_calendar_events_calendar_uid_unique
            ON caldav.calendar_events(calendar_id, ical_uid);
    END IF;
END $$;

CREATE OR REPLACE FUNCTION caldav.touch_calendar_from_event_change()
RETURNS trigger AS $$
DECLARE
    affected_calendar_id UUID;
BEGIN
    affected_calendar_id := COALESCE(NEW.calendar_id, OLD.calendar_id);

    UPDATE caldav.calendars
    SET
        updated_at = CURRENT_TIMESTAMP,
        ctag = ((extract(epoch FROM clock_timestamp()) * 1000000)::bigint)::text
    WHERE id = affected_calendar_id;

    RETURN COALESCE(NEW, OLD);
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_calendar_events_touch_calendar_insert
    ON caldav.calendar_events;

DROP TRIGGER IF EXISTS trg_calendar_events_touch_calendar_update
    ON caldav.calendar_events;

DROP TRIGGER IF EXISTS trg_calendar_events_touch_calendar_delete
    ON caldav.calendar_events;

CREATE TRIGGER trg_calendar_events_touch_calendar_insert
AFTER INSERT ON caldav.calendar_events
FOR EACH ROW
EXECUTE FUNCTION caldav.touch_calendar_from_event_change();

CREATE TRIGGER trg_calendar_events_touch_calendar_update
AFTER UPDATE ON caldav.calendar_events
FOR EACH ROW
WHEN (
    OLD.resource_name IS DISTINCT FROM NEW.resource_name
    OR OLD.ical_uid IS DISTINCT FROM NEW.ical_uid
    OR OLD.ical_data IS DISTINCT FROM NEW.ical_data
    OR OLD.etag IS DISTINCT FROM NEW.etag
    OR OLD.summary IS DISTINCT FROM NEW.summary
    OR OLD.description IS DISTINCT FROM NEW.description
    OR OLD.location IS DISTINCT FROM NEW.location
    OR OLD.start_time IS DISTINCT FROM NEW.start_time
    OR OLD.end_time IS DISTINCT FROM NEW.end_time
    OR OLD.all_day IS DISTINCT FROM NEW.all_day
    OR OLD.rrule IS DISTINCT FROM NEW.rrule
)
EXECUTE FUNCTION caldav.touch_calendar_from_event_change();

CREATE TRIGGER trg_calendar_events_touch_calendar_delete
AFTER DELETE ON caldav.calendar_events
FOR EACH ROW
EXECUTE FUNCTION caldav.touch_calendar_from_event_change();
