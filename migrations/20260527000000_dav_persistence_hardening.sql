-- Harden DAV persistence for client-compatible CalDAV/CardDAV discovery,
-- object addressing, ctag/sync metadata, and tombstones.
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE SCHEMA IF NOT EXISTS caldav;
CREATE SCHEMA IF NOT EXISTS carddav;

ALTER TABLE IF EXISTS caldav.calendars ALTER COLUMN id SET DEFAULT gen_random_uuid();
ALTER TABLE IF EXISTS caldav.calendar_events ALTER COLUMN id SET DEFAULT gen_random_uuid();
ALTER TABLE IF EXISTS caldav.calendars
    ADD COLUMN IF NOT EXISTS display_name TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS sync_version BIGINT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS supported_components TEXT[] NOT NULL DEFAULT ARRAY['VEVENT', 'VTODO']::TEXT[],
    ADD COLUMN IF NOT EXISTS timezone TEXT,
    ADD COLUMN IF NOT EXISTS calendar_order INTEGER NOT NULL DEFAULT 0;
ALTER TABLE IF EXISTS caldav.calendars ALTER COLUMN ctag SET DEFAULT '1';
ALTER TABLE IF EXISTS caldav.calendars ALTER COLUMN color SET DEFAULT '#1f78d1ff';
UPDATE caldav.calendars SET display_name = name WHERE display_name = '';
UPDATE caldav.calendars SET color = '#1f78d1ff' WHERE color IS NULL OR btrim(color) = '';
UPDATE caldav.calendars SET sync_version = GREATEST(1, ctag::BIGINT) WHERE ctag ~ '^[0-9]+$';
UPDATE caldav.calendars SET ctag = sync_version::TEXT WHERE ctag IS NULL OR ctag !~ '^[0-9]+$';
UPDATE caldav.calendars SET ctag = sync_version::TEXT WHERE ctag ~ '^[0-9]+$' AND ctag::BIGINT < 1;

DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_caldav_calendars_display_name_not_empty' AND conrelid = 'caldav.calendars'::regclass) THEN
        ALTER TABLE caldav.calendars ADD CONSTRAINT chk_caldav_calendars_display_name_not_empty CHECK (btrim(display_name) <> '') NOT VALID;
    END IF;
END; $$;
ALTER TABLE IF EXISTS caldav.calendars VALIDATE CONSTRAINT chk_caldav_calendars_display_name_not_empty;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_caldav_calendars_sync_version_positive' AND conrelid = 'caldav.calendars'::regclass) THEN
        ALTER TABLE caldav.calendars ADD CONSTRAINT chk_caldav_calendars_sync_version_positive CHECK (sync_version >= 1) NOT VALID;
    END IF;
END; $$;
ALTER TABLE IF EXISTS caldav.calendars VALIDATE CONSTRAINT chk_caldav_calendars_sync_version_positive;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_caldav_calendars_supported_components_not_empty' AND conrelid = 'caldav.calendars'::regclass) THEN
        ALTER TABLE caldav.calendars ADD CONSTRAINT chk_caldav_calendars_supported_components_not_empty CHECK (array_length(supported_components, 1) >= 1) NOT VALID;
    END IF;
END; $$;
ALTER TABLE IF EXISTS caldav.calendars VALIDATE CONSTRAINT chk_caldav_calendars_supported_components_not_empty;
CREATE INDEX IF NOT EXISTS idx_caldav_calendars_owner_name ON caldav.calendars(owner_id, name);
CREATE INDEX IF NOT EXISTS idx_caldav_calendars_owner_sync_version ON caldav.calendars(owner_id, sync_version);
CREATE INDEX IF NOT EXISTS idx_caldav_calendars_updated_at ON caldav.calendars(updated_at DESC);
INSERT INTO caldav.calendars (id, name, display_name, owner_id, description, color, is_public, ctag, sync_version, supported_components, calendar_order)
SELECT gen_random_uuid(), 'personal', 'Personal', u.id, NULL, '#1f78d1ff', FALSE, '1', 1, ARRAY['VEVENT', 'VTODO']::TEXT[], 0
FROM auth.users u
WHERE NOT EXISTS (SELECT 1 FROM caldav.calendars c WHERE c.owner_id = u.id AND c.name = 'personal');

ALTER TABLE IF EXISTS caldav.calendar_events
    ADD COLUMN IF NOT EXISTS resource_name TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS component_type TEXT NOT NULL DEFAULT 'VEVENT',
    ADD COLUMN IF NOT EXISTS sync_version BIGINT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
UPDATE caldav.calendar_events SET resource_name = id::TEXT || '.ics' WHERE resource_name IS NULL OR btrim(resource_name) = '';
UPDATE caldav.calendar_events SET component_type = 'VTODO' WHERE component_type = 'VEVENT' AND ical_data ILIKE '%BEGIN:VTODO%';
UPDATE caldav.calendar_events SET component_type = 'VEVENT' WHERE component_type IS NULL OR btrim(component_type) = '' OR component_type NOT IN ('VEVENT', 'VTODO');
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_calendar_events_resource_name_not_empty' AND conrelid = 'caldav.calendar_events'::regclass) THEN
        ALTER TABLE caldav.calendar_events ADD CONSTRAINT chk_calendar_events_resource_name_not_empty CHECK (btrim(resource_name) <> '') NOT VALID;
    END IF;
END; $$;
ALTER TABLE IF EXISTS caldav.calendar_events VALIDATE CONSTRAINT chk_calendar_events_resource_name_not_empty;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_calendar_events_component_type' AND conrelid = 'caldav.calendar_events'::regclass) THEN
        ALTER TABLE caldav.calendar_events ADD CONSTRAINT chk_calendar_events_component_type CHECK (component_type IN ('VEVENT', 'VTODO')) NOT VALID;
    END IF;
END; $$;
ALTER TABLE IF EXISTS caldav.calendar_events VALIDATE CONSTRAINT chk_calendar_events_component_type;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_calendar_events_sync_version_positive' AND conrelid = 'caldav.calendar_events'::regclass) THEN
        ALTER TABLE caldav.calendar_events ADD CONSTRAINT chk_calendar_events_sync_version_positive CHECK (sync_version >= 1) NOT VALID;
    END IF;
END; $$;
ALTER TABLE IF EXISTS caldav.calendar_events VALIDATE CONSTRAINT chk_calendar_events_sync_version_positive;
CREATE UNIQUE INDEX IF NOT EXISTS idx_calendar_events_calendar_resource_name_active ON caldav.calendar_events(calendar_id, resource_name) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_calendar_events_calendar_component_time ON caldav.calendar_events(calendar_id, component_type, start_time, end_time) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_calendar_events_calendar_sync_version ON caldav.calendar_events(calendar_id, sync_version) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_calendar_events_updated_at ON caldav.calendar_events(updated_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_calendar_events_deleted_at ON caldav.calendar_events(deleted_at) WHERE deleted_at IS NOT NULL;
CREATE TABLE IF NOT EXISTS caldav.calendar_object_tombstones (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(), calendar_id UUID NOT NULL, resource_name TEXT NOT NULL,
    ical_uid TEXT, deleted_etag VARCHAR(64), deleted_at TIMESTAMPTZ NOT NULL DEFAULT now(), sync_version BIGINT NOT NULL DEFAULT 1,
    UNIQUE(calendar_id, resource_name)
);
CREATE INDEX IF NOT EXISTS idx_calendar_tombstones_calendar_sync ON caldav.calendar_object_tombstones(calendar_id, sync_version);
CREATE INDEX IF NOT EXISTS idx_calendar_tombstones_deleted_at ON caldav.calendar_object_tombstones(deleted_at);

ALTER TABLE IF EXISTS carddav.address_books ALTER COLUMN id SET DEFAULT gen_random_uuid();
ALTER TABLE IF EXISTS carddav.contacts ALTER COLUMN id SET DEFAULT gen_random_uuid();
ALTER TABLE IF EXISTS carddav.address_books ADD COLUMN IF NOT EXISTS sync_version BIGINT NOT NULL DEFAULT 1, ADD COLUMN IF NOT EXISTS addressbook_order INTEGER NOT NULL DEFAULT 0;
ALTER TABLE IF EXISTS carddav.address_books ALTER COLUMN ctag SET DEFAULT '1';
UPDATE carddav.address_books SET display_name = initcap(replace(name, '-', ' ')) WHERE display_name IS NULL OR btrim(display_name) = '';
UPDATE carddav.address_books SET sync_version = GREATEST(1, ctag::BIGINT) WHERE ctag ~ '^[0-9]+$';
UPDATE carddav.address_books SET ctag = sync_version::TEXT WHERE ctag IS NULL OR ctag !~ '^[0-9]+$';
UPDATE carddav.address_books SET ctag = sync_version::TEXT WHERE ctag ~ '^[0-9]+$' AND ctag::BIGINT < 1;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_carddav_address_books_display_name_not_empty' AND conrelid = 'carddav.address_books'::regclass) THEN
        ALTER TABLE carddav.address_books ADD CONSTRAINT chk_carddav_address_books_display_name_not_empty CHECK (display_name IS NOT NULL AND btrim(display_name) <> '') NOT VALID;
    END IF;
END; $$;
ALTER TABLE IF EXISTS carddav.address_books VALIDATE CONSTRAINT chk_carddav_address_books_display_name_not_empty;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_carddav_address_books_sync_version_positive' AND conrelid = 'carddav.address_books'::regclass) THEN
        ALTER TABLE carddav.address_books ADD CONSTRAINT chk_carddav_address_books_sync_version_positive CHECK (sync_version >= 1) NOT VALID;
    END IF;
END; $$;
ALTER TABLE IF EXISTS carddav.address_books VALIDATE CONSTRAINT chk_carddav_address_books_sync_version_positive;
CREATE INDEX IF NOT EXISTS idx_carddav_address_books_owner_name ON carddav.address_books(owner_id, name);
CREATE INDEX IF NOT EXISTS idx_carddav_address_books_owner_sync_version ON carddav.address_books(owner_id, sync_version);
CREATE INDEX IF NOT EXISTS idx_carddav_address_books_updated_at ON carddav.address_books(updated_at DESC);
INSERT INTO carddav.address_books (id, name, owner_id, display_name, description, ctag, sync_version, addressbook_order)
SELECT gen_random_uuid(), 'contacts', u.id, 'Contacts', NULL, '1', 1, 0 FROM auth.users u
WHERE NOT EXISTS (SELECT 1 FROM carddav.address_books ab WHERE ab.owner_id = u.id AND ab.name = 'contacts');

ALTER TABLE IF EXISTS carddav.contacts ADD COLUMN IF NOT EXISTS resource_name TEXT NOT NULL DEFAULT '', ADD COLUMN IF NOT EXISTS sync_version BIGINT NOT NULL DEFAULT 1, ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
UPDATE carddav.contacts SET resource_name = id::TEXT || '.vcf' WHERE resource_name IS NULL OR btrim(resource_name) = '';
UPDATE carddav.contacts SET version = '4.0' WHERE version IS NULL OR btrim(version) = '';
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_contacts_resource_name_not_empty' AND conrelid = 'carddav.contacts'::regclass) THEN
        ALTER TABLE carddav.contacts ADD CONSTRAINT chk_contacts_resource_name_not_empty CHECK (btrim(resource_name) <> '') NOT VALID;
    END IF;
END; $$;
ALTER TABLE IF EXISTS carddav.contacts VALIDATE CONSTRAINT chk_contacts_resource_name_not_empty;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_contacts_vcard_version' AND conrelid = 'carddav.contacts'::regclass) THEN
        ALTER TABLE carddav.contacts ADD CONSTRAINT chk_contacts_vcard_version CHECK (version IN ('3.0', '4.0')) NOT VALID;
    END IF;
END; $$;
ALTER TABLE IF EXISTS carddav.contacts VALIDATE CONSTRAINT chk_contacts_vcard_version;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'chk_contacts_sync_version_positive' AND conrelid = 'carddav.contacts'::regclass) THEN
        ALTER TABLE carddav.contacts ADD CONSTRAINT chk_contacts_sync_version_positive CHECK (sync_version >= 1) NOT VALID;
    END IF;
END; $$;
ALTER TABLE IF EXISTS carddav.contacts VALIDATE CONSTRAINT chk_contacts_sync_version_positive;
CREATE UNIQUE INDEX IF NOT EXISTS idx_contacts_address_book_resource_name_active ON carddav.contacts(address_book_id, resource_name) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_contacts_address_book_sync_version ON carddav.contacts(address_book_id, sync_version) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_contacts_updated_at ON carddav.contacts(updated_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_contacts_deleted_at ON carddav.contacts(deleted_at) WHERE deleted_at IS NOT NULL;
CREATE TABLE IF NOT EXISTS carddav.contact_tombstones (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(), address_book_id UUID NOT NULL, resource_name TEXT NOT NULL,
    uid TEXT, deleted_etag VARCHAR(64), deleted_at TIMESTAMPTZ NOT NULL DEFAULT now(), sync_version BIGINT NOT NULL DEFAULT 1,
    UNIQUE(address_book_id, resource_name)
);
CREATE INDEX IF NOT EXISTS idx_contact_tombstones_book_sync ON carddav.contact_tombstones(address_book_id, sync_version);
CREATE INDEX IF NOT EXISTS idx_contact_tombstones_deleted_at ON carddav.contact_tombstones(deleted_at);

CREATE OR REPLACE FUNCTION caldav.bump_calendar_sync_metadata(p_calendar_id UUID) RETURNS BIGINT AS $$
DECLARE v_sync_version BIGINT;
BEGIN
    UPDATE caldav.calendars SET sync_version = sync_version + 1, ctag = (sync_version + 1)::TEXT, updated_at = now()
    WHERE id = p_calendar_id RETURNING sync_version INTO v_sync_version;
    RETURN v_sync_version;
END; $$ LANGUAGE plpgsql;
CREATE OR REPLACE FUNCTION caldav.track_calendar_event_sync() RETURNS TRIGGER AS $$
DECLARE v_sync_version BIGINT;
BEGIN
    IF TG_OP = 'DELETE' THEN
        v_sync_version := caldav.bump_calendar_sync_metadata(OLD.calendar_id);
        IF v_sync_version IS NOT NULL THEN
            INSERT INTO caldav.calendar_object_tombstones (calendar_id, resource_name, ical_uid, deleted_etag, deleted_at, sync_version)
            VALUES (OLD.calendar_id, OLD.resource_name, OLD.ical_uid, OLD.etag, now(), v_sync_version)
            ON CONFLICT (calendar_id, resource_name) DO UPDATE SET ical_uid = EXCLUDED.ical_uid, deleted_etag = EXCLUDED.deleted_etag, deleted_at = EXCLUDED.deleted_at, sync_version = EXCLUDED.sync_version;
        END IF;
        RETURN OLD;
    END IF;
    IF TG_OP = 'UPDATE' AND OLD.calendar_id IS DISTINCT FROM NEW.calendar_id THEN PERFORM caldav.bump_calendar_sync_metadata(OLD.calendar_id); END IF;
    PERFORM caldav.bump_calendar_sync_metadata(NEW.calendar_id);
    RETURN NEW;
END; $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_track_calendar_event_sync ON caldav.calendar_events;
CREATE TRIGGER trg_track_calendar_event_sync AFTER INSERT OR UPDATE OR DELETE ON caldav.calendar_events FOR EACH ROW EXECUTE FUNCTION caldav.track_calendar_event_sync();

CREATE OR REPLACE FUNCTION carddav.bump_address_book_sync_metadata(p_address_book_id UUID) RETURNS BIGINT AS $$
DECLARE v_sync_version BIGINT;
BEGIN
    UPDATE carddav.address_books SET sync_version = sync_version + 1, ctag = (sync_version + 1)::TEXT, updated_at = now()
    WHERE id = p_address_book_id RETURNING sync_version INTO v_sync_version;
    RETURN v_sync_version;
END; $$ LANGUAGE plpgsql;
CREATE OR REPLACE FUNCTION carddav.track_contact_sync() RETURNS TRIGGER AS $$
DECLARE v_sync_version BIGINT;
BEGIN
    IF TG_OP = 'DELETE' THEN
        v_sync_version := carddav.bump_address_book_sync_metadata(OLD.address_book_id);
        IF v_sync_version IS NOT NULL THEN
            INSERT INTO carddav.contact_tombstones (address_book_id, resource_name, uid, deleted_etag, deleted_at, sync_version)
            VALUES (OLD.address_book_id, OLD.resource_name, OLD.uid, OLD.etag, now(), v_sync_version)
            ON CONFLICT (address_book_id, resource_name) DO UPDATE SET uid = EXCLUDED.uid, deleted_etag = EXCLUDED.deleted_etag, deleted_at = EXCLUDED.deleted_at, sync_version = EXCLUDED.sync_version;
        END IF;
        RETURN OLD;
    END IF;
    IF TG_OP = 'UPDATE' AND OLD.address_book_id IS DISTINCT FROM NEW.address_book_id THEN PERFORM carddav.bump_address_book_sync_metadata(OLD.address_book_id); END IF;
    PERFORM carddav.bump_address_book_sync_metadata(NEW.address_book_id);
    RETURN NEW;
END; $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_track_contact_sync ON carddav.contacts;
CREATE TRIGGER trg_track_contact_sync AFTER INSERT OR UPDATE OR DELETE ON carddav.contacts FOR EACH ROW EXECUTE FUNCTION carddav.track_contact_sync();
