# ==============================================================================
# MKCALENDAR Test Suite - Story 1
# ==============================================================================

# 1. User should be able to create a new calendar successfully
MKCALENDAR {{base_url}}/caldav/{{username}}/work-calendar/
Authorization: Basic {{basic_auth_credentials}}
Content-Type: application/xml; charset=utf-8
