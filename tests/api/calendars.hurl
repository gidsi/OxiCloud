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


# ─────────────────────────────────────────────────────────────
# Step 9 – PUT an event to the calendar (Create)
# ─────────────────────────────────────────────────────────────
PUT {{base_url}}/caldav/{{username}}/work-calendar/event1.ics
[BasicAuth]
{{username}}: {{dav_password}}
Content-Type: text/calendar; charset=utf-8
`BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//OxiCloud//NONSGML v1.0//EN
BEGIN:VEVENT
UID:event1
DTSTAMP:20230526T000000Z
DTSTART:20230526T100000Z
DTEND:20230526T110000Z
SUMMARY:Test Event
END:VEVENT
END:VCALENDAR`

HTTP 201
[Captures]
event_etag: header "ETag"
[Asserts]
header "ETag" exists


# ─────────────────────────────────────────────────────────────
# Step 10 – GET the event to verify creation
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/caldav/{{username}}/work-calendar/event1.ics
[BasicAuth]
{{username}}: {{dav_password}}

HTTP 200
[Asserts]
header "Content-Type" contains "text/calendar"
header "ETag" == {{event_etag}}
body contains "SUMMARY:Test Event"


# ─────────────────────────────────────────────────────────────
# Step 11 – PUT update the event successfully with matching If-Match
# ─────────────────────────────────────────────────────────────
PUT {{base_url}}/caldav/{{username}}/work-calendar/event1.ics
[BasicAuth]
{{username}}: {{dav_password}}
If-Match: {{event_etag}}
Content-Type: text/calendar; charset=utf-8
`BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//OxiCloud//NONSGML v1.0//EN
BEGIN:VEVENT
UID:event1
DTSTAMP:20230526T000000Z
DTSTART:20230526T100000Z
DTEND:20230526T110000Z
SUMMARY:Updated Test Event
END:VEVENT
END:VCALENDAR`

HTTP 204
[Captures]
new_event_etag: header "ETag"
[Asserts]
header "ETag" exists


# ─────────────────────────────────────────────────────────────
# Step 12 – GET the event to verify update
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/caldav/{{username}}/work-calendar/event1.ics
[BasicAuth]
{{username}}: {{dav_password}}

HTTP 200
[Asserts]
header "Content-Type" contains "text/calendar"
header "ETag" == {{new_event_etag}}
body contains "SUMMARY:Updated Test Event"


# ─────────────────────────────────────────────────────────────
# Step 13 – PUT update fails with incorrect If-Match (Concurrency Control)
# ─────────────────────────────────────────────────────────────
PUT {{base_url}}/caldav/{{username}}/work-calendar/event1.ics
[BasicAuth]
{{username}}: {{dav_password}}
If-Match: "wrong-etag"
Content-Type: text/calendar; charset=utf-8
`BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//OxiCloud//NONSGML v1.0//EN
BEGIN:VEVENT
UID:event1
DTSTAMP:20230526T000000Z
DTSTART:20230526T100000Z
DTEND:20230526T110000Z
SUMMARY:Hacked Event
END:VEVENT
END:VCALENDAR`

HTTP 412


# ─────────────────────────────────────────────────────────────
# Step 14 – PUT create fails if event already exists and If-None-Match: * (Concurrency Control)
# ─────────────────────────────────────────────────────────────
PUT {{base_url}}/caldav/{{username}}/work-calendar/event1.ics
[BasicAuth]
{{username}}: {{dav_password}}
If-None-Match: *
Content-Type: text/calendar; charset=utf-8
`BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//OxiCloud//NONSGML v1.0//EN
BEGIN:VEVENT
UID:event1
DTSTAMP:20230526T000000Z
DTSTART:20230526T100000Z
DTEND:20230526T110000Z
SUMMARY:Another Event
END:VEVENT
END:VCALENDAR`

HTTP 412


# ─────────────────────────────────────────────────────────────
# Step 15 – PUT an event to the calendar to be deleted
# ─────────────────────────────────────────────────────────────
PUT {{base_url}}/caldav/{{username}}/work-calendar/event-to-delete.ics
[BasicAuth]
{{username}}: {{dav_password}}
Content-Type: text/calendar; charset=utf-8
`BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//OxiCloud//NONSGML v1.0//EN
BEGIN:VEVENT
UID:event-to-delete
DTSTAMP:20230526T000000Z
DTSTART:20230526T100000Z
DTEND:20230526T110000Z
SUMMARY:Delete Me Event
END:VEVENT
END:VCALENDAR`

HTTP 201
[Asserts]
header "ETag" exists


# ─────────────────────────────────────────────────────────────
# Step 16 – DELETE the event
# ─────────────────────────────────────────────────────────────
DELETE {{base_url}}/caldav/{{username}}/work-calendar/event-to-delete.ics
[BasicAuth]
{{username}}: {{dav_password}}

HTTP 204


# ─────────────────────────────────────────────────────────────
# Step 17 – GET the deleted event to verify it is gone (HTTP 404)
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/caldav/{{username}}/work-calendar/event-to-delete.ics
[BasicAuth]
{{username}}: {{dav_password}}

HTTP 404
