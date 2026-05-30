# ==============================================================================
# MKCALENDAR Test Suite - Story 1
# ==============================================================================

# Get an app password
POST {{base_url}}/api/auth/login
Content-Type: application/json
{
  "username": "{{username}}",
  "password": "{{password}}"
}
HTTP 200
[Captures]
token: jsonpath "$.access_token"

POST {{base_url}}/api/auth/app-passwords
Authorization: Bearer {{token}}
Content-Type: application/json
{
  "label": "Test Client"
}
HTTP 200
[Captures]
app_password: jsonpath "$.password"

# 1. User should be able to create a new calendar successfully
MKCALENDAR {{base_url}}/caldav/{{username}}/work-calendar/
[BasicAuth]
{{username}}: {{app_password}}
Content-Type: application/xml; charset=utf-8
HTTP 201