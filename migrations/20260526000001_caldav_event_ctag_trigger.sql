CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE SCHEMA IF NOT EXISTS caldav;

ALTER TABLE IF EXISTS caldav.calendars
    ADD COLUMN IF NOT EXISTS ctag VARCHAR(64);

ALTER TABLE IF EXISTS caldav.calendars
    ALTER COLUMN ctag SET DEFAULT encode(gen_random_bytes(16), 'hex');

DO $$
BEGIN
    IF to_regclass('caldav.calendars') IS NOT NULL THEN
        UPDATE caldav.calendars
        SET ctag = encode(gen_random_bytes(16), 'hex')
        WHERE ctag IS NULL
           OR btrim(ctag) = '';
    END IF;
END $$;

ALTER TABLE IF EXISTS caldav.calendars
    ALTER COLUMN ctag SET NOT NULL;

DO $$
BEGIN
    IF to_regclass('caldav.calendars') IS NOT NULL
       AND NOT EXISTS (
            SELECT 1
            FROM pg_constraint
            WHERE conname = 'chk_calendars_ctag_not_empty'
              AND conrelid = 'caldav.calendars'::regclass
       )
    THEN
        ALTER TABLE caldav.calendars
            ADD CONSTRAINT chk_calendars_ctag_not_empty
            CHECK (btrim(ctag) <> '');
    END IF;
END $$;

CREATE OR REPLACE FUNCTION caldav.bump_calendar_ctag_for_event_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND OLD.calendar_id IS DISTINCT FROM NEW.calendar_id THEN
        UPDATE caldav.calendars
        SET ctag = encode(gen_random_bytes(16), 'hex'),
            updated_at = CURRENT_TIMESTAMP
        WHERE id = OLD.calendar_id;

        UPDATE caldav.calendars
        SET ctag = encode(gen_random_bytes(16), 'hex'),
            updated_at = CURRENT_TIMESTAMP
        WHERE id = NEW.calendar_id;

        RETURN NEW;
    END IF;

    UPDATE caldav.calendars
    SET ctag = encode(gen_random_bytes(16), 'hex'),
        updated_at = CURRENT_TIMESTAMP
    WHERE id = CASE
        WHEN TG_OP = 'DELETE' THEN OLD.calendar_id
        ELSE NEW.calendar_id
    END;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;

    RETURN NEW;
END;
$$;

DO $$
BEGIN
    IF to_regclass('caldav.calendar_events') IS NOT NULL
       AND to_regclass('caldav.calendars') IS NOT NULL
    THEN
        DROP TRIGGER IF EXISTS trg_calendar_events_bump_calendar_ctag
            ON caldav.calendar_events;

        CREATE TRIGGER trg_calendar_events_bump_calendar_ctag
            AFTER INSERT OR UPDATE OR DELETE ON caldav.calendar_events
            FOR EACH ROW
            EXECUTE FUNCTION caldav.bump_calendar_ctag_for_event_change();
    END IF;
END $$;

COMMENT ON FUNCTION caldav.bump_calendar_ctag_for_event_change() IS
    'Bumps parent calendar ctag and updated_at after calendar event insert, update, or delete so CalDAV clients detect collection changes.';
