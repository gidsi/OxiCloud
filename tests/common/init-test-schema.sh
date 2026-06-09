#!/usr/bin/env bash
# Apply every migration in lexical order to a test database, then seed
# the minimum `auth.users` row that integration tests need.
#
# Connection parameters come from the libpq env vars (PGHOST, PGPORT,
# PGUSER, PGPASSWORD, PGDATABASE) so the same script works against:
#
#   - the local docker-compose-test postgres on port 5433
#     (PGHOST=localhost PGPORT=5433 PGUSER=oxicloud_test
#      PGPASSWORD=oxicloud_test PGDATABASE=oxicloud_test)
#
#   - the CI postgres service on port 5432
#     (PGHOST=localhost PGPORT=5432 PGUSER=postgres
#      PGPASSWORD=postgres PGDATABASE=oxicloud_test)
#
# The seed users are placeholders — password hashes are not validated.
# These users are used by Hurl E2E tests (e.g., auth_login.hurl expects 'admin').

set -euo pipefail

: "${PGHOST:?PGHOST must be set}"
: "${PGPORT:?PGPORT must be set}"
: "${PGUSER:?PGUSER must be set}"
: "${PGPASSWORD:?PGPASSWORD must be set}"
: "${PGDATABASE:?PGDATABASE must be set}"
export PGHOST PGPORT PGUSER PGPASSWORD PGDATABASE

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

echo "[init-schema] applying migrations to ${PGUSER}@${PGHOST}:${PGPORT}/${PGDATABASE}"
for f in "${REPO_ROOT}"/migrations/*.sql; do
    echo "[init-schema]   $(basename "$f")"
    psql -v ON_ERROR_STOP=1 -f "$f" >/dev/null
done

echo "[init-schema] seeding test users (idempotent)"
psql -v ON_ERROR_STOP=1 -c "
    INSERT INTO auth.users (username, email, password_hash, role)
    VALUES ('admin', 'admin@example.test', 'placeholder-not-validated', 'admin'),
           ('user1', 'user1@example.test', 'placeholder-not-validated', 'user'),
           ('user2', 'user2@example.test', 'placeholder-not-validated', 'user')
    ON CONFLICT (username) DO NOTHING;
" >/dev/null

echo "[init-schema] done"
