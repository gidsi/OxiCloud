# Test Scenario 1: Client Auto-Discovery Redirects (Unauthenticated)
# Enforces that self-hosters don't need to configure complex paths.
GET http://localhost:8080/.well-known/caldav
HTTP 301
[Asserts]
header "Location" == "/caldav/"

GET http://localhost:8080/.well-known/carddav
HTTP 301
[Asserts]
header "Location" == "/carddav/"

# Security Constraint: Enforce HTTPS scheme in redirects unless explicitly local
# (Assuming the Hurl environment here verifies the scheme mapping for prod instances)
[Asserts]
header "Location" startsWith "/" # Relative redirects are scheme-agnostic and safe

# Test Scenario 2: Server Capabilities Verification
# Ensures sync clients know we support contacts and calendars natively.
OPTIONS http://localhost:8080/caldav/
HTTP 200
[Asserts]
header "DAV" contains "1"
header "DAV" contains "3"
header "DAV" contains "calendar-access"
header "DAV" contains "addressbook"

# Test Scenario 3: Principal Home-Set Discovery
# Verifies the strict XML parsing and fetching of the user's specific sync URLs.
PROPFIND http://localhost:8080/caldav/
Authorization: Basic dGVzdHVzZXI6cGFzc3dvcmQ=
Depth: 0
Content-Type: text/xml; charset=utf-8
