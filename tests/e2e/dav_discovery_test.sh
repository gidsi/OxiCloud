#!/bin/bash
set -euo pipefail

echo "Running E2E Scenario: DAVx5, Thunderbird, and Apple Calendar discovery flow..."

DOMAIN=${1:-localhost:8080}
USERNAME=${2:-admin}
PASSWORD=${3:-TestPassword1!}
BASE_URL="${base_url:-${BASE_URL:-http://$DOMAIN}}"

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

http_status() {
    tail -n1
}

http_body() {
    sed '$d'
}

echo "Using BASE_URL=$BASE_URL"

echo "Ensuring initial admin account exists..."
SETUP_RESPONSE=$(curl -s -w "\n%{http_code}" \
    -X POST "$BASE_URL/api/setup" \
    -H "Content-Type: application/json" \
    --data-binary "{\"username\":\"$USERNAME\",\"email\":\"admin@example.com\",\"password\":\"$PASSWORD\"}")

SETUP_STATUS=$(echo "$SETUP_RESPONSE" | http_status)
if [ "$SETUP_STATUS" != "201" ] && [ "$SETUP_STATUS" != "403" ]; then
    echo "$SETUP_RESPONSE"
    fail "Expected 201 Created or 403 already initialized from /api/setup, got $SETUP_STATUS"
fi

echo "Testing /.well-known/caldav redirect..."
CALDAV_HEADERS=$(curl -s -i -o /dev/null -D - "$BASE_URL/.well-known/caldav")
CALDAV_STATUS=$(printf '%s\n' "$CALDAV_HEADERS" | awk 'tolower($0) ~ /^http\// { code=$2 } END { print code }')
CALDAV_LOCATION=$(printf '%s\n' "$CALDAV_HEADERS" | awk 'tolower($0) ~ /^location:/ { sub(/\r$/, ""); print $2 }' | tail -n1)

if [ "$CALDAV_STATUS" != "308" ] && [ "$CALDAV_STATUS" != "301" ] && [ "$CALDAV_STATUS" != "302" ]; then
    printf '%s\n' "$CALDAV_HEADERS"
    fail "Expected redirect for /.well-known/caldav, got $CALDAV_STATUS"
fi

if [ "$CALDAV_LOCATION" != "/caldav/" ]; then
    printf '%s\n' "$CALDAV_HEADERS"
    fail "Expected Location: /caldav/ for /.well-known/caldav, got $CALDAV_LOCATION"
fi

echo "Testing /.well-known/carddav redirect..."
CARDDAV_HEADERS=$(curl -s -i -o /dev/null -D - "$BASE_URL/.well-known/carddav")
CARDDAV_STATUS=$(printf '%s\n' "$CARDDAV_HEADERS" | awk 'tolower($0) ~ /^http\// { code=$2 } END { print code }')
CARDDAV_LOCATION=$(printf '%s\n' "$CARDDAV_HEADERS" | awk 'tolower($0) ~ /^location:/ { sub(/\r$/, ""); print $2 }' | tail -n1)

if [ "$CARDDAV_STATUS" != "308" ] && [ "$CARDDAV_STATUS" != "301" ] && [ "$CARDDAV_STATUS" != "302" ]; then
    printf '%s\n' "$CARDDAV_HEADERS"
    fail "Expected redirect for /.well-known/carddav, got $CARDDAV_STATUS"
fi

if [ "$CARDDAV_LOCATION" != "/carddav/" ]; then
    printf '%s\n' "$CARDDAV_HEADERS"
    fail "Expected Location: /carddav/ for /.well-known/carddav, got $CARDDAV_LOCATION"
fi

echo "Testing OPTIONS capabilities..."
OPTIONS_HEADERS=$(curl -s -i -o /dev/null -D - \
    -u "$USERNAME:$PASSWORD" \
    -X OPTIONS "$BASE_URL/caldav/")

OPTIONS_STATUS=$(printf '%s\n' "$OPTIONS_HEADERS" | awk 'tolower($0) ~ /^http\// { code=$2 } END { print code }')
DAV_HEADER=$(printf '%s\n' "$OPTIONS_HEADERS" | grep -i "^DAV:" | tr -d '\r' || true)
ALLOW_HEADER=$(printf '%s\n' "$OPTIONS_HEADERS" | grep -i "^Allow:" | tr -d '\r' || true)

if [ "$OPTIONS_STATUS" != "200" ] && [ "$OPTIONS_STATUS" != "204" ]; then
    printf '%s\n' "$OPTIONS_HEADERS"
    fail "Expected 200 or 204 for OPTIONS /caldav/, got $OPTIONS_STATUS"
fi

if [[ "$DAV_HEADER" != *"calendar-access"* ]]; then
    printf '%s\n' "$OPTIONS_HEADERS"
    fail "Missing calendar-access in DAV header: $DAV_HEADER"
fi

if [[ "$DAV_HEADER" != *"addressbook"* ]]; then
    printf '%s\n' "$OPTIONS_HEADERS"
    fail "Missing addressbook in DAV header: $DAV_HEADER"
fi

if [[ "$ALLOW_HEADER" != *"OPTIONS"* ]] || [[ "$ALLOW_HEADER" != *"PROPFIND"* ]] || [[ "$ALLOW_HEADER" != *"REPORT"* ]]; then
    printf '%s\n' "$OPTIONS_HEADERS"
    fail "Allow header does not advertise required DAV methods: $ALLOW_HEADER"
fi

echo "Testing unauthenticated PROPFIND challenge..."
UNAUTH_HEADERS=$(curl -s -i -o /dev/null -D - \
    -X PROPFIND "$BASE_URL/caldav/" \
    -H "Depth: 0")

UNAUTH_STATUS=$(printf '%s\n' "$UNAUTH_HEADERS" | awk 'tolower($0) ~ /^http\// { code=$2 } END { print code }')
AUTH_HEADER=$(printf '%s\n' "$UNAUTH_HEADERS" | grep -i "^WWW-Authenticate:" | tr -d '\r' || true)

if [ "$UNAUTH_STATUS" != "401" ]; then
    printf '%s\n' "$UNAUTH_HEADERS"
    fail "Expected 401 Unauthorized for unauthenticated PROPFIND, got $UNAUTH_STATUS"
fi

if [[ "$AUTH_HEADER" != *"Basic realm=\"OxiCloud\""* ]]; then
    printf '%s\n' "$UNAUTH_HEADERS"
    fail "Missing or incorrect WWW-Authenticate Basic challenge: $AUTH_HEADER"
fi

echo "Testing authenticated principal discovery..."
PRINCIPAL_RESPONSE=$(curl -s -w "\n%{http_code}" \
    -X PROPFIND "$BASE_URL/caldav/" \
    -u "$USERNAME:$PASSWORD" \
    -H "Depth: 0" \
    -H "Content-Type: application/xml; charset=utf-8" \
    --data-binary '<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav" xmlns:CARD="urn:ietf:params:xml:ns:carddav">
  <D:prop>
    <D:current-user-principal/>
    <D:principal-collection-set/>
    <C:calendar-home-set/>
    <CARD:addressbook-home-set/>
  </D:prop>
</D:propfind>')

PRINCIPAL_STATUS=$(echo "$PRINCIPAL_RESPONSE" | http_status)
PRINCIPAL_BODY=$(echo "$PRINCIPAL_RESPONSE" | http_body)

if [ "$PRINCIPAL_STATUS" != "207" ]; then
    echo "$PRINCIPAL_BODY"
    fail "Expected 207 Multi-Status for principal PROPFIND, got $PRINCIPAL_STATUS"
fi

if [[ "$PRINCIPAL_BODY" != *"current-user-principal"* ]]; then
    echo "$PRINCIPAL_BODY"
    fail "Principal discovery response missing current-user-principal"
fi

if [[ "$PRINCIPAL_BODY" != *"calendar-home-set"* ]]; then
    echo "$PRINCIPAL_BODY"
    fail "Principal discovery response missing calendar-home-set"
fi

if [[ "$PRINCIPAL_BODY" != *"addressbook-home-set"* ]]; then
    echo "$PRINCIPAL_BODY"
    fail "Principal discovery response missing addressbook-home-set"
fi

echo "Testing invalid credentials rejection..."
INVALID_RESPONSE=$(curl -s -w "\n%{http_code}" \
    -X PROPFIND "$BASE_URL/caldav/" \
    -u "invalid:wrong" \
    -H "Depth: 0" \
    -H "Content-Type: application/xml; charset=utf-8" \
    --data-binary '<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:">
  <D:prop>
    <D:current-user-principal/>
  </D:prop>
</D:propfind>')

INVALID_STATUS=$(echo "$INVALID_RESPONSE" | http_status)
INVALID_BODY=$(echo "$INVALID_RESPONSE" | http_body)

if [ "$INVALID_STATUS" != "401" ]; then
    echo "$INVALID_BODY"
    fail "Expected 401 Unauthorized for invalid credentials, got $INVALID_STATUS"
fi

if [[ "$INVALID_BODY" == *"<html"* ]]; then
    echo "$INVALID_BODY"
    fail "Server returned HTML instead of DAV-compatible unauthorized response"
fi

echo "DAV client discovery E2E verification succeeded."
exit 0
