-- ============================================================
-- OxiCloud CalDAV Story 4: DELETE calendar collection support
--
-- Ensure database-level referential integrity for recursive
-- WebDAV collection deletes:
--   DELETE /caldav/{username}/{calendar-path}/
-- ============================================================

CREATE SCHEMA IF NOT EXISTS caldav;

DO $$
BEGIN
    IF to_regclass('caldav.calendar_events') IS NOT NULL
       AND to_regclass('caldav.calendars') IS NOT NULL
    THEN
        DELETE FROM caldav.calendar_events AS event
        WHERE NOT EXISTS (
            SELECT 1
            FROM caldav.calendars AS calendar
            WHERE calendar.id = event.calendar_id
        );
    END IF;
END $$;

DO $$
BEGIN
    IF to_regclass('caldav.calendar_shares') IS NOT NULL
       AND to_regclass('caldav.calendars') IS NOT NULL
    THEN
        DELETE FROM caldav.calendar_shares AS share
        WHERE NOT EXISTS (
            SELECT 1
            FROM caldav.calendars AS calendar
            WHERE calendar.id = share.calendar_id
        );
    END IF;
END $$;

DO $$
BEGIN
    IF to_regclass('caldav.calendar_properties') IS NOT NULL
       AND to_regclass('caldav.calendars') IS NOT NULL
    THEN
        DELETE FROM caldav.calendar_properties AS property
        WHERE NOT EXISTS (
            SELECT 1
            FROM caldav.calendars AS calendar
            WHERE calendar.id = property.calendar_id
        );
    END IF;
END $$;

ALTER TABLE IF EXISTS caldav.calendar_events
    ALTER COLUMN calendar_id SET NOT NULL;

ALTER TABLE IF EXISTS caldav.calendar_shares
    ALTER COLUMN calendar_id SET NOT NULL;

ALTER TABLE IF EXISTS caldav.calendar_properties
    ALTER COLUMN calendar_id SET NOT NULL;

DO $$
DECLARE
    existing_constraint_name TEXT;
BEGIN
    IF to_regclass('caldav.calendar_events') IS NOT NULL
       AND to_regclass('caldav.calendars') IS NOT NULL
       AND NOT EXISTS (
            SELECT 1
            FROM pg_constraint AS c
            WHERE c.conrelid = 'caldav.calendar_events'::regclass
              AND c.confrelid = 'caldav.calendars'::regclass
              AND c.contype = 'f'
              AND c.confdeltype = 'c'
              AND EXISTS (
                    SELECT 1
                    FROM unnest(c.conkey) AS key(attnum)
                    JOIN pg_attribute AS a
                      ON a.attrelid = c.conrelid
                     AND a.attnum = key.attnum
                    WHERE a.attname = 'calendar_id'
              )
       )
    THEN
        FOR existing_constraint_name IN
            SELECT c.conname
            FROM pg_constraint AS c
            WHERE c.conrelid = 'caldav.calendar_events'::regclass
              AND c.confrelid = 'caldav.calendars'::regclass
              AND c.contype = 'f'
              AND EXISTS (
                    SELECT 1
                    FROM unnest(c.conkey) AS key(attnum)
                    JOIN pg_attribute AS a
                      ON a.attrelid = c.conrelid
                     AND a.attnum = key.attnum
                    WHERE a.attname = 'calendar_id'
              )
        LOOP
            EXECUTE format(
                'ALTER TABLE caldav.calendar_events DROP CONSTRAINT IF EXISTS %I',
                existing_constraint_name
            );
        END LOOP;

        ALTER TABLE caldav.calendar_events
            ADD CONSTRAINT fk_calendar_events_calendar_id_cascade
            FOREIGN KEY (calendar_id)
            REFERENCES caldav.calendars(id)
            ON DELETE CASCADE
            NOT VALID;

        ALTER TABLE caldav.calendar_events
            VALIDATE CONSTRAINT fk_calendar_events_calendar_id_cascade;
    END IF;
END $$;

DO $$
DECLARE
    existing_constraint_name TEXT;
BEGIN
    IF to_regclass('caldav.calendar_shares') IS NOT NULL
       AND to_regclass('caldav.calendars') IS NOT NULL
       AND NOT EXISTS (
            SELECT 1
            FROM pg_constraint AS c
            WHERE c.conrelid = 'caldav.calendar_shares'::regclass
              AND c.confrelid = 'caldav.calendars'::regclass
              AND c.contype = 'f'
              AND c.confdeltype = 'c'
              AND EXISTS (
                    SELECT 1
                    FROM unnest(c.conkey) AS key(attnum)
                    JOIN pg_attribute AS a
                      ON a.attrelid = c.conrelid
                     AND a.attnum = key.attnum
                    WHERE a.attname = 'calendar_id'
              )
       )
    THEN
        FOR existing_constraint_name IN
            SELECT c.conname
            FROM pg_constraint AS c
            WHERE c.conrelid = 'caldav.calendar_shares'::regclass
              AND c.confrelid = 'caldav.calendars'::regclass
              AND c.contype = 'f'
              AND EXISTS (
                    SELECT 1
                    FROM unnest(c.conkey) AS key(attnum)
                    JOIN pg_attribute AS a
                      ON a.attrelid = c.conrelid
                     AND a.attnum = key.attnum
                    WHERE a.attname = 'calendar_id'
              )
        LOOP
            EXECUTE format(
                'ALTER TABLE caldav.calendar_shares DROP CONSTRAINT IF EXISTS %I',
                existing_constraint_name
            );
        END LOOP;

        ALTER TABLE caldav.calendar_shares
            ADD CONSTRAINT fk_calendar_shares_calendar_id_cascade
            FOREIGN KEY (calendar_id)
            REFERENCES caldav.calendars(id)
            ON DELETE CASCADE
            NOT VALID;

        ALTER TABLE caldav.calendar_shares
            VALIDATE CONSTRAINT fk_calendar_shares_calendar_id_cascade;
    END IF;
END $$;

DO $$
DECLARE
    existing_constraint_name TEXT;
BEGIN
    IF to_regclass('caldav.calendar_properties') IS NOT NULL
       AND to_regclass('caldav.calendars') IS NOT NULL
       AND NOT EXISTS (
            SELECT 1
            FROM pg_constraint AS c
            WHERE c.conrelid = 'caldav.calendar_properties'::regclass
              AND c.confrelid = 'caldav.calendars'::regclass
              AND c.contype = 'f'
              AND c.confdeltype = 'c'
              AND EXISTS (
                    SELECT 1
                    FROM unnest(c.conkey) AS key(attnum)
                    JOIN pg_attribute AS a
                      ON a.attrelid = c.conrelid
                     AND a.attnum = key.attnum
                    WHERE a.attname = 'calendar_id'
              )
       )
    THEN
        FOR existing_constraint_name IN
            SELECT c.conname
            FROM pg_constraint AS c
            WHERE c.conrelid = 'caldav.calendar_properties'::regclass
              AND c.confrelid = 'caldav.calendars'::regclass
              AND c.contype = 'f'
              AND EXISTS (
                    SELECT 1
                    FROM unnest(c.conkey) AS key(attnum)
                    JOIN pg_attribute AS a
                      ON a.attrelid = c.conrelid
                     AND a.attnum = key.attnum
                    WHERE a.attname = 'calendar_id'
              )
        LOOP
            EXECUTE format(
                'ALTER TABLE caldav.calendar_properties DROP CONSTRAINT IF EXISTS %I',
                existing_constraint_name
            );
        END LOOP;

        ALTER TABLE caldav.calendar_properties
            ADD CONSTRAINT fk_calendar_properties_calendar_id_cascade
            FOREIGN KEY (calendar_id)
            REFERENCES caldav.calendars(id)
            ON DELETE CASCADE
            NOT VALID;

        ALTER TABLE caldav.calendar_properties
            VALIDATE CONSTRAINT fk_calendar_properties_calendar_id_cascade;
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_calendar_events_calendar_resource_path
    ON caldav.calendar_events(calendar_id, resource_path);
