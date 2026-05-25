-- Unified DAV discovery principal home-set paths.
--
-- This migration adds DAV metadata used by the application layer to fetch
-- canonical principal, calendar home-set, and address book home-set paths for
-- an authenticated user ID.

CREATE SCHEMA IF NOT EXISTS dav;

CREATE TABLE IF NOT EXISTS dav.principals (
    user_id UUID PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE
);

ALTER TABLE dav.principals
    ADD COLUMN IF NOT EXISTS username TEXT NOT NULL DEFAULT '';

ALTER TABLE dav.principals
    ADD COLUMN IF NOT EXISTS principal_path TEXT NOT NULL DEFAULT '/caldav/principals/';

ALTER TABLE dav.principals
    ADD COLUMN IF NOT EXISTS calendar_home_set_path TEXT NOT NULL DEFAULT '/caldav/';

ALTER TABLE dav.principals
    ADD COLUMN IF NOT EXISTS addressbook_home_set_path TEXT NOT NULL DEFAULT '/carddav/';

ALTER TABLE dav.principals
    ADD COLUMN IF NOT EXISTS created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP;

ALTER TABLE dav.principals
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP;

CREATE UNIQUE INDEX IF NOT EXISTS idx_dav_principals_user_id
    ON dav.principals(user_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_dav_principals_principal_path
    ON dav.principals(principal_path);

CREATE UNIQUE INDEX IF NOT EXISTS idx_dav_principals_calendar_home_set_path
    ON dav.principals(calendar_home_set_path);

CREATE UNIQUE INDEX IF NOT EXISTS idx_dav_principals_addressbook_home_set_path
    ON dav.principals(addressbook_home_set_path);

CREATE INDEX IF NOT EXISTS idx_dav_principals_username
    ON dav.principals(username);

INSERT INTO dav.principals (
    user_id,
    username,
    principal_path,
    calendar_home_set_path,
    addressbook_home_set_path,
    created_at,
    updated_at
)
SELECT
    u.id,
    u.username,
    '/caldav/principals/' || u.username || '/',
    '/caldav/' || u.username || '/',
    '/carddav/' || u.username || '/',
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
FROM auth.users u
ON CONFLICT (user_id) DO UPDATE
SET
    username = EXCLUDED.username,
    principal_path = EXCLUDED.principal_path,
    calendar_home_set_path = EXCLUDED.calendar_home_set_path,
    addressbook_home_set_path = EXCLUDED.addressbook_home_set_path,
    updated_at = CURRENT_TIMESTAMP;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_dav_principals_username_not_empty'
          AND conrelid = 'dav.principals'::regclass
    ) THEN
        ALTER TABLE dav.principals
            ADD CONSTRAINT chk_dav_principals_username_not_empty
            CHECK (char_length(username) > 0);
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_dav_principals_principal_path_format'
          AND conrelid = 'dav.principals'::regclass
    ) THEN
        ALTER TABLE dav.principals
            ADD CONSTRAINT chk_dav_principals_principal_path_format
            CHECK (
                principal_path LIKE '/caldav/principals/%/'
                AND principal_path NOT LIKE '%//%'
            );
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_dav_principals_calendar_home_set_path_format'
          AND conrelid = 'dav.principals'::regclass
    ) THEN
        ALTER TABLE dav.principals
            ADD CONSTRAINT chk_dav_principals_calendar_home_set_path_format
            CHECK (
                calendar_home_set_path LIKE '/caldav/%/'
                AND calendar_home_set_path NOT LIKE '%//%'
            );
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_dav_principals_addressbook_home_set_path_format'
          AND conrelid = 'dav.principals'::regclass
    ) THEN
        ALTER TABLE dav.principals
            ADD CONSTRAINT chk_dav_principals_addressbook_home_set_path_format
            CHECK (
                addressbook_home_set_path LIKE '/carddav/%/'
                AND addressbook_home_set_path NOT LIKE '%//%'
            );
    END IF;
END $$;

CREATE OR REPLACE FUNCTION dav.set_principals_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_dav_principals_updated_at ON dav.principals;

CREATE TRIGGER trg_dav_principals_updated_at
BEFORE UPDATE ON dav.principals
FOR EACH ROW
EXECUTE FUNCTION dav.set_principals_updated_at();

CREATE OR REPLACE FUNCTION dav.sync_principal_home_sets_from_user()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO dav.principals (
        user_id,
        username,
        principal_path,
        calendar_home_set_path,
        addressbook_home_set_path,
        created_at,
        updated_at
    )
    VALUES (
        NEW.id,
        NEW.username,
        '/caldav/principals/' || NEW.username || '/',
        '/caldav/' || NEW.username || '/',
        '/carddav/' || NEW.username || '/',
        CURRENT_TIMESTAMP,
        CURRENT_TIMESTAMP
    )
    ON CONFLICT (user_id) DO UPDATE
    SET
        username = EXCLUDED.username,
        principal_path = EXCLUDED.principal_path,
        calendar_home_set_path = EXCLUDED.calendar_home_set_path,
        addressbook_home_set_path = EXCLUDED.addressbook_home_set_path,
        updated_at = CURRENT_TIMESTAMP;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_auth_users_sync_dav_principals_insert ON auth.users;
DROP TRIGGER IF EXISTS trg_auth_users_sync_dav_principals_username_update ON auth.users;

CREATE TRIGGER trg_auth_users_sync_dav_principals_insert
AFTER INSERT ON auth.users
FOR EACH ROW
EXECUTE FUNCTION dav.sync_principal_home_sets_from_user();

CREATE TRIGGER trg_auth_users_sync_dav_principals_username_update
AFTER UPDATE OF username ON auth.users
FOR EACH ROW
WHEN (OLD.username IS DISTINCT FROM NEW.username)
EXECUTE FUNCTION dav.sync_principal_home_sets_from_user();

COMMENT ON SCHEMA dav IS 'DAV protocol metadata used for WebDAV, CalDAV, and CardDAV discovery';

COMMENT ON TABLE dav.principals IS
    'Canonical DAV principal and home-set paths for authenticated users';

COMMENT ON COLUMN dav.principals.user_id IS
    'Authenticated OxiCloud user ID; one DAV principal record per user';

COMMENT ON COLUMN dav.principals.username IS
    'Username snapshot used to construct stable DAV discovery paths';

COMMENT ON COLUMN dav.principals.principal_path IS
    'Canonical current-user-principal URL returned in DAV discovery PROPFIND responses';

COMMENT ON COLUMN dav.principals.calendar_home_set_path IS
    'Canonical CalDAV calendar-home-set URL returned in DAV discovery PROPFIND responses';

COMMENT ON COLUMN dav.principals.addressbook_home_set_path IS
    'Canonical CardDAV addressbook-home-set URL returned in DAV discovery PROPFIND responses';
