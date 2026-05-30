-- CalDAV MKCALENDAR persistence support.
--
-- Adds stable per-owner calendar slugs for URI-based CalDAV collections and
-- normalizes calendar dead-property storage used by MKCALENDAR/PROPPATCH.

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE SCHEMA IF NOT EXISTS caldav;

ALTER TABLE caldav.calendars
    ADD COLUMN IF NOT EXISTS slug TEXT;

WITH candidates AS (
    SELECT
        id,
        owner_id,
        COALESCE(
            NULLIF(
                btrim(
                    regexp_replace(
                        regexp_replace(
                            lower(trim(COALESCE(NULLIF(slug, ''), name, id::text))),
                            '[^a-z0-9._~-]+',
                            '-',
                            'g'
                        ),
                        '-+',
                        '-',
                        'g'
                    ),
                    '-'
                ),
                ''
            ),
            'calendar-' || substring(id::text, 1, 8)
        ) AS base_slug,
        created_at
    FROM caldav.calendars
),
ranked AS (
    SELECT
        id,
        CASE
            WHEN row_number() OVER (PARTITION BY owner_id, base_slug ORDER BY created_at, id) = 1
                THEN base_slug
            ELSE base_slug || '-' || row_number() OVER (PARTITION BY owner_id, base_slug ORDER BY created_at, id)
        END AS unique_slug
    FROM candidates
)
UPDATE caldav.calendars c
SET slug = ranked.unique_slug
FROM ranked
WHERE c.id = ranked.id
  AND (c.slug IS NULL OR c.slug = '');

ALTER TABLE caldav.calendars
    ALTER COLUMN slug SET NOT NULL;

ALTER TABLE caldav.calendars
    ALTER COLUMN slug SET DEFAULT ('calendar-' || uuid_generate_v4()::text);

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_calendars_slug_not_empty'
          AND conrelid = 'caldav.calendars'::regclass
    ) THEN
        ALTER TABLE caldav.calendars
            ADD CONSTRAINT chk_calendars_slug_not_empty
            CHECK (char_length(slug) > 0);
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_calendars_slug_no_path_segments'
          AND conrelid = 'caldav.calendars'::regclass
    ) THEN
        ALTER TABLE caldav.calendars
            ADD CONSTRAINT chk_calendars_slug_no_path_segments
            CHECK (slug NOT LIKE '%/%' AND slug <> '.' AND slug <> '..');
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_calendars_color_hex_format'
          AND conrelid = 'caldav.calendars'::regclass
    ) THEN
        ALTER TABLE caldav.calendars
            ADD CONSTRAINT chk_calendars_color_hex_format
            CHECK (
                color IS NULL
                OR color ~ '^#[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?$'
            )
            NOT VALID;
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_calendars_owner_slug_unique
    ON caldav.calendars(owner_id, slug);

CREATE INDEX IF NOT EXISTS idx_calendars_owner_name
    ON caldav.calendars(owner_id, name);

CREATE INDEX IF NOT EXISTS idx_calendars_owner_updated_at
    ON caldav.calendars(owner_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS caldav.calendar_properties (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    calendar_id UUID NOT NULL REFERENCES caldav.calendars(id) ON DELETE CASCADE,
    name VARCHAR(255),
    value TEXT
);

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'caldav'
          AND table_name = 'calendar_properties'
          AND column_name = 'property_name'
    )
    AND NOT EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'caldav'
          AND table_name = 'calendar_properties'
          AND column_name = 'name'
    ) THEN
        ALTER TABLE caldav.calendar_properties
            RENAME COLUMN property_name TO name;
    END IF;

    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'caldav'
          AND table_name = 'calendar_properties'
          AND column_name = 'property_value'
    )
    AND NOT EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'caldav'
          AND table_name = 'calendar_properties'
          AND column_name = 'value'
    ) THEN
        ALTER TABLE caldav.calendar_properties
            RENAME COLUMN property_value TO value;
    END IF;
END $$;

ALTER TABLE caldav.calendar_properties
    ADD COLUMN IF NOT EXISTS name VARCHAR(255);

ALTER TABLE caldav.calendar_properties
    ADD COLUMN IF NOT EXISTS value TEXT;

UPDATE caldav.calendar_properties
SET name = 'legacy-property-' || id::text
WHERE name IS NULL OR name = '';

UPDATE caldav.calendar_properties
SET value = ''
WHERE value IS NULL;

ALTER TABLE caldav.calendar_properties
    ALTER COLUMN name SET NOT NULL;

ALTER TABLE caldav.calendar_properties
    ALTER COLUMN value SET NOT NULL;

ALTER TABLE caldav.calendar_properties
    ALTER COLUMN value SET DEFAULT '';

ALTER TABLE caldav.calendar_properties
    DROP COLUMN IF EXISTS property_name;

ALTER TABLE caldav.calendar_properties
    DROP COLUMN IF EXISTS property_value;

CREATE UNIQUE INDEX IF NOT EXISTS idx_calendar_properties_calendar_id_name_unique
    ON caldav.calendar_properties(calendar_id, name);

CREATE INDEX IF NOT EXISTS idx_calendar_properties_name
    ON caldav.calendar_properties(name);

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_calendar_properties_name_not_empty'
          AND conrelid = 'caldav.calendar_properties'::regclass
    ) THEN
        ALTER TABLE caldav.calendar_properties
            ADD CONSTRAINT chk_calendar_properties_name_not_empty
            CHECK (char_length(name) > 0);
    END IF;
END $$;

INSERT INTO caldav.calendar_properties (calendar_id, name, value)
SELECT c.id, '{DAV:}displayname', c.name
FROM caldav.calendars c
ON CONFLICT (calendar_id, name) DO UPDATE
SET value = EXCLUDED.value;

INSERT INTO caldav.calendar_properties (calendar_id, name, value)
SELECT c.id, '{DAV:}resourcetype', 'collection,calendar'
FROM caldav.calendars c
ON CONFLICT (calendar_id, name) DO NOTHING;

INSERT INTO caldav.calendar_properties (calendar_id, name, value)
SELECT c.id, '{urn:ietf:params:xml:ns:caldav}supported-calendar-component-set', 'VEVENT'
FROM caldav.calendars c
ON CONFLICT (calendar_id, name) DO NOTHING;

INSERT INTO caldav.calendar_properties (calendar_id, name, value)
SELECT c.id, '{urn:ietf:params:xml:ns:caldav}calendar-description', c.description
FROM caldav.calendars c
WHERE c.description IS NOT NULL
ON CONFLICT (calendar_id, name) DO UPDATE
SET value = EXCLUDED.value;

INSERT INTO caldav.calendar_properties (calendar_id, name, value)
SELECT c.id, '{http://apple.com/ns/ical/}calendar-color', c.color
FROM caldav.calendars c
WHERE c.color IS NOT NULL
  AND c.color <> ''
ON CONFLICT (calendar_id, name) DO UPDATE
SET value = EXCLUDED.value;

COMMENT ON COLUMN caldav.calendars.slug IS
    'Stable per-owner CalDAV collection slug used in /caldav/{username}/{slug}/ URLs';

COMMENT ON INDEX caldav.idx_calendars_owner_slug_unique IS
    'Enforces one calendar collection per owner and CalDAV URI slug';

COMMENT ON TABLE caldav.calendar_properties IS
    'Custom and protocol WebDAV/CalDAV dead properties stored for calendar collections';

COMMENT ON COLUMN caldav.calendar_properties.name IS
    'Property name, preferably in expanded Clark notation such as {DAV:}displayname';

COMMENT ON COLUMN caldav.calendar_properties.value IS
    'Serialized property value used for WebDAV/CalDAV PROPFIND and PROPPATCH responses';
