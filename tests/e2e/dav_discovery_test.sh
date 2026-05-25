#!/bin/bash
set -e

echo "Running E2E Scenario 4: Simulating DAVx5 and Apple Calendar auto-discovery flows..."

DOMAIN=${1:-localhost:8080}
USERNAME=${2:-testuser}
PASSWORD=${3:-password}

# 1. Test .well-known redirect (Auto-Discovery)
echo "Testing /.well-known/caldav redirect..."
STATUS=$(curl -s -o /dev/null -w "%{http_code}" "http://$DOMAIN/.well-known/caldav")
if [ "$STATUS" != "301" ] && [ "$STATUS" != "302" ]; then
    echo "FAIL: Expected 301/302 for /.well-known/caldav, got $STATUS"
    exit 1
fi

# 2. Test Capabilities Verification
echo "Testing OPTIONS capabilities..."
DAV_HEADER=$(curl -s -I -u "$USERNAME:$PASSWORD" -X OPTIONS "http://$DOMAIN/caldav/" | grep -i "^DAV:" | tr -d '\r')
if [[ "$DAV_HEADER" != *"calendar-access"* ]] || [[ "$DAV_HEADER" != *"addressbook"* ]] || [[ "$DAV_HEADER" != *"3"* ]]; then
    echo "FAIL: Missing required DAV capabilities in OPTIONS response. Got: $DAV_HEADER"
    exit 1
fi

# 3. Test Principal Discovery Strictness
echo "Testing PROPFIND Principal Home-Set..."
RESPONSE=$(curl -s -w "\n%{http_code}" -X PROPFIND -u "$USERNAME:$PASSWORD" \
    -H "Depth: 0" \
    -H "Content-Type: text/xml; charset=utf-8" \
    -d '<?xml version="1.0" encoding="utf-8" ?><d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:card="urn:ietf:params:xml:ns:carddav"><d:prop><d:current-user-principal /><c:calendar-home-set /><card:addressbook-home-set /></d:prop></d:propfind>' \
    "http://$DOMAIN/caldav/")

HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
BODY=$(echo "$RESPONSE" | sed '$d')

if [ "$HTTP_CODE" != "207" ]; then
    echo "FAIL: Expected 207 Multi-Status for PROPFIND, got $HTTP_CODE"
    exit 1
fi

if [[ "$BODY" != *"calendar-home-set"* ]] || [[ "$BODY" != *"addressbook-home-set"* ]]; then
    echo "FAIL: XML response missing home-set URLs required to auto-map calendars and contacts."
    echo "Response body: $BODY"
    exit 1
fi

# 4. Security Constraint: Rate Limiting
echo "Testing Rate Limiting on OPTIONS to prevent Reconnaissance..."
HTTP_429_SEEN=false
for i in {1..100}; do
    STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X OPTIONS "http://$DOMAIN/caldav/")
    if [ "$STATUS" == "429" ]; then
        HTTP_429_SEEN=true
        break
    fi
done

if [ "$HTTP_429_SEEN" = false ]; then
    echo "FAIL: Rate limiting not enforced on DAV root after 100 requests."
    exit 1
fi

# 5. Invalid Credentials Rejection
echo "Testing Invalid Credentials on PROPFIND..."
RESPONSE=$(curl -s -w "\n%{http_code}" -X PROPFIND -u "invalid:wrong" \
    -H "Depth: 0" \
    -H "Content-Type: text/xml; charset=utf-8" \
    -d '<?xml version="1.0" encoding="utf-8" ?><d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:card="urn:ietf:params:xml:ns:carddav"><d:prop><d:current-user-principal /><c:calendar-home-set /><card:addressbook-home-set /></d:prop></d:propfind>' \
    "http://$DOMAIN/caldav/")

HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
BODY=$(echo "$RESPONSE" | sed '$d')

if [ "$HTTP_CODE" != "401" ]; then
    echo "FAIL: Expected 401 Unauthorized for invalid credentials, got $HTTP_CODE"
    exit 1
fi

if [[ "$BODY" == *"<html"* ]]; then
    echo "FAIL: Server returned HTML login page instead of proper DAV response for invalid credentials."
    exit 1
fi

AUTH_HEADER=$(curl -s -I -X PROPFIND -u "invalid:wrong" "http://$DOMAIN/caldav/" | grep -i "^WWW-Authenticate:" | tr -d '\r')
if [[ "$AUTH_HEADER" != *"Basic realm=\"OxiCloud\""* ]]; then
    echo "FAIL: Missing or incorrect WWW-Authenticate header. Got: $AUTH_HEADER"
    exit 1
fi

echo "Scenario 4: E2E Integration Success! Auto-discovery flows completely verified."
exit 0
