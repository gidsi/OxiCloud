# Test Scenario 1: Client Auto-Discovery Redirects (Unauthenticated)
# Enforces that self-hosters don't need to configure complex paths.
GET {{base_url}}/.well-known/caldav
HTTP 301
[Asserts]
header "Location" == "/caldav/"
header "Location" startsWith "/"

GET {{base_url}}/.well-known/carddav
HTTP 301
[Asserts]
header "Location" == "/carddav/"
header "Location" startsWith "/"

# Test Scenario 2: Server Capabilities Verification with valid credentials
# Ensures sync clients know we support contacts and calendars natively.
OPTIONS {{base_url}}/caldav/
Authorization: Basic dGVzdHVzZXI6cGFzc3dvcmQ=
HTTP 200
[Asserts]
header "DAV" contains "1"
header "DAV" contains "3"
header "DAV" contains "calendar-access"
header "DAV" contains "addressbook"

# Test Scenario 3: Principal Home-Set Discovery
# Verifies the strict XML parsing and fetching of the user's specific sync URLs.
PROPFIND {{base_url}}/caldav/
Authorization: Basic dGVzdHVzZXI6cGFzc3dvcmQ=
Depth: 0
Content-Type: text/xml; charset=utf-8
HTTP 207
[Asserts]
header "Content-Type" contains "xml"
body contains "calendar-home-set"
body contains "addressbook-home-set"

# Test Scenario 4: Missing Credentials on protected endpoints (caldav)
OPTIONS {{base_url}}/caldav/
HTTP 401
[Asserts]
header "WWW-Authenticate" == "Basic realm=\"OxiCloud\""
body not contains "<html"
body not contains "<!DOCTYPE html"

PROPFIND {{base_url}}/caldav/
Depth: 0
Content-Type: text/xml; charset=utf-8
HTTP 401
[Asserts]
header "WWW-Authenticate" == "Basic realm=\"OxiCloud\""
body not contains "<html"
body not contains "<!DOCTYPE html"

# Test Scenario 4: Missing Credentials on protected endpoints (carddav)
OPTIONS {{base_url}}/carddav/
HTTP 401
[Asserts]
header "WWW-Authenticate" == "Basic realm=\"OxiCloud\""
body not contains "<html"
body not contains "<!DOCTYPE html"

PROPFIND {{base_url}}/carddav/
Depth: 0
Content-Type: text/xml; charset=utf-8
HTTP 401
[Asserts]
header "WWW-Authenticate" == "Basic realm=\"OxiCloud\""
body not contains "<html"
body not contains "<!DOCTYPE html"

# Test Scenario 5: Invalid Credentials during PROPFIND
PROPFIND {{base_url}}/caldav/
Authorization: Basic aW52YWxpZDp3cm9uZw==
Depth: 0
Content-Type: text/xml; charset=utf-8
HTTP 401
[Asserts]
header "WWW-Authenticate" == "Basic realm=\"OxiCloud\""
body not contains "<html"
body not contains "<!DOCTYPE html"
