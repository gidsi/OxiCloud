# ==============================================================================
# OxiCloud – CalDAV MKCALENDAR integration scenario
# ==============================================================================
# Run:
#   hurl --variables-file tests/api/test.env --test tests/api/setup.hurl tests/api/calendars.hurl
#
# Variables required (see test.env):
#   base_url   – e.g. http://localhost:8087
#   username   – OxiCloud username
#   password   – OxiCloud password
# ==============================================================================


# ─────────────────────────────────────────────────────────────
# Step 1 – Unauthenticated MKCALENDAR is rejected
# ─────────────────────────────────────────────────────────────
MKCALENDAR {{base_url}}/caldav/{{username}}/work-calendar/
Content-Type: text/xml; charset=utf-8
`<?xml version="1.0" encoding="UTF-8"?>
<C:mkcalendar xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:set>
    <D:prop>
      <D:displayname>Work Calendar</D:displayname>
    </D:prop>
  </D:set>
</C:mkcalendar>`

HTTP 401
[Asserts]
header "WWW-Authenticate" == "Basic realm=\"OxiCloud\""


# ─────────────────────────────────────────────────────────────
# Step 2 – Login with the primary API credentials
# ─────────────────────────────────────────────────────────────
POST {{base_url}}/api/auth/login
Content-Type: application/json
{
  "username": "{{username}}",
  "password": "{{password}}"
}

HTTP 200
[Captures]
token: jsonpath "$.access_token"
[Asserts]
jsonpath "$.access_token" isString
jsonpath "$.token_type" == "Bearer"


# ─────────────────────────────────────────────────────────────
# Step 3 – Create an app password for DAV Basic authentication
# ─────────────────────────────────────────────────────────────
POST {{base_url}}/api/auth/app-passwords
Authorization: Bearer {{token}}
Content-Type: application/json
{
  "label": "Hurl CalDAV MKCALENDAR test",
  "scopes": "caldav",
  "expires_in_days": 1
}

HTTP 200
[Captures]
dav_password: jsonpath "$.password"
[Asserts]
jsonpath "$.username" == "{{username}}"
jsonpath "$.password" isString
jsonpath "$.scopes" == "caldav"


# ─────────────────────────────────────────────────────────────
# Step 4 – Authenticated MKCALENDAR creates a direct child calendar
# ─────────────────────────────────────────────────────────────
MKCALENDAR {{base_url}}/caldav/{{username}}/work-calendar/
[BasicAuth]
{{username}}: {{dav_password}}
Content-Type: text/xml; charset=utf-8
`<?xml version="1.0" encoding="UTF-8"?>
<C:mkcalendar xmlns:D="DAV:"
              xmlns:C="urn:ietf:params:xml:ns:caldav"
              xmlns:A="http://apple.com/ns/ical/">
  <D:set>
    <D:prop>
      <D:displayname>Work Calendar</D:displayname>
      <C:calendar-description>Calendar created by the MKCALENDAR Hurl test</C:calendar-description>
      <A:calendar-color>#2952CC</A:calendar-color>
    </D:prop>
  </D:set>
</C:mkcalendar>`

HTTP 201
[Asserts]
header "Location" == "/caldav/{{username}}/work-calendar/"
header "Cache-Control" == "no-cache"


# ─────────────────────────────────────────────────────────────
# Step 5 – Duplicate MKCALENDAR returns conflict
# ─────────────────────────────────────────────────────────────
MKCALENDAR {{base_url}}/caldav/{{username}}/work-calendar/
[BasicAuth]
{{username}}: {{dav_password}}
Content-Type: text/xml; charset=utf-8
`<?xml version="1.0" encoding="UTF-8"?>
<C:mkcalendar xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:set>
    <D:prop>
      <D:displayname>Work Calendar</D:displayname>
    </D:prop>
  </D:set>
</C:mkcalendar>`

HTTP 409


# ─────────────────────────────────────────────────────────────
# Step 6 – Nested MKCALENDAR targets are rejected
# ─────────────────────────────────────────────────────────────
MKCALENDAR {{base_url}}/caldav/{{username}}/work-calendar/nested/
[BasicAuth]
{{username}}: {{dav_password}}
Content-Type: text/xml; charset=utf-8
`<?xml version="1.0" encoding="UTF-8"?>
<C:mkcalendar xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:set>
    <D:prop>
      <D:displayname>Nested Calendar</D:displayname>
    </D:prop>
  </D:set>
</C:mkcalendar>`

HTTP 409


# ─────────────────────────────────────────────────────────────
# Step 7 – A user cannot create calendars in another user's home
# ─────────────────────────────────────────────────────────────
MKCALENDAR {{base_url}}/caldav/other-user/work-calendar/
[BasicAuth]
{{username}}: {{dav_password}}
Content-Type: text/xml; charset=utf-8
`<?xml version="1.0" encoding="UTF-8"?>
<C:mkcalendar xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:set>
    <D:prop>
      <D:displayname>Forbidden Calendar</D:displayname>
    </D:prop>
  </D:set>
</C:mkcalendar>`

HTTP 403


# ─────────────────────────────────────────────────────────────
# Step 8 – PROPFIND discovers the calendar created by MKCALENDAR
# ─────────────────────────────────────────────────────────────
PROPFIND {{base_url}}/caldav/{{username}}/
[BasicAuth]
{{username}}: {{dav_password}}
Depth: 1
Content-Type: text/xml; charset=utf-8
`<?xml version="1.0" encoding="UTF-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
  <D:prop>
    <D:displayname/>
    <D:resourcetype/>
    <C:calendar-description/>
    <C:supported-calendar-component-set/>
  </D:prop>
</D:propfind>`

HTTP 207
[Asserts]
header "Content-Type" contains "xml"
body contains "/caldav/{{username}}/work-calendar/"
body contains "Work Calendar"
body contains "Calendar created by the MKCALENDAR Hurl test"
