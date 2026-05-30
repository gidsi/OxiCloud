# =============================================================
# OxiCloud – Recent Items API end-to-end scenario
# =============================================================
# Run:
#   hurl --variables-file tests/api/test.env --test tests/api/recent.hurl
# =============================================================

# ─────────────────────────────────────────────────────────────
# Step 1 – Login and capture the JWT token
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


# ─────────────────────────────────────────────────────────────
# Step 2 – No recent items yet
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/api/recent
Authorization: Bearer {{token}}

HTTP 200
[Asserts]
jsonpath "$" isCollection
jsonpath "$" count == 0


# ─────────────────────────────────────────────────────────────
# Step 2.5 – Create folders and upload a file
# ─────────────────────────────────────────────────────────────
POST {{base_url}}/api/folders
Authorization: Bearer {{token}}
Content-Type: application/json
{
  "name": "test1"
}

HTTP 201
[Captures]
test1_id: jsonpath "$.id"

POST {{base_url}}/api/folders
Authorization: Bearer {{token}}
Content-Type: application/json
{
  "name": "test2"
}

HTTP 201
[Captures]
test2_id: jsonpath "$.id"

POST {{base_url}}/api/files/upload
Authorization: Bearer {{token}}
[MultipartFormData]
folder_id: {{test2_id}}
file: file,fixtures/hello.txt; text/plain

HTTP 201
[Captures]
file_id: jsonpath "$.id"

# ─────────────────────────────────────────────────────────────
# Step 3 – Check item existence (simulate what files-folders did)
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/api/folders
Authorization: Bearer {{token}}

HTTP 200
[Captures]
home_folder_id: jsonpath "$[0].id"

GET {{base_url}}/api/folders/{{home_folder_id}}/contents
Authorization: Bearer {{token}}

HTTP 200
[Asserts]
jsonpath "$[0].name" == "test1"
jsonpath "$[1].name" == "test2"

GET {{base_url}}/api/files?folder_id={{test2_id}}
Authorization: Bearer {{token}}

HTTP 200
[Asserts]
jsonpath "$[0].name" == "hello.txt"


# ─────────────────────────────────────────────────────────────
# Step 4 – Record folder test1 as recently accessed
# ─────────────────────────────────────────────────────────────
POST {{base_url}}/api/recent/folder/{{test1_id}}
Authorization: Bearer {{token}}

HTTP 200


# ─────────────────────────────────────────────────────────────
# Step 5 – Record file hello.txt as recently accessed
# ─────────────────────────────────────────────────────────────
POST {{base_url}}/api/recent/file/{{file_id}}
Authorization: Bearer {{token}}

HTTP 200


# ─────────────────────────────────────────────────────────────
# Step 6 – Recent contains both items, most recent first (file)
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/api/recent
Authorization: Bearer {{token}}

HTTP 200
[Asserts]
jsonpath "$" count == 2
jsonpath "$[0].item_id" == {{file_id}}
jsonpath "$[0].item_type" == "file"
jsonpath "$[1].item_id" == {{test1_id}}
jsonpath "$[1].item_type" == "folder"


# ─────────────────────────────────────────────────────────────
# Step 7 – Record folder test1 again (should bump it to top)
# ─────────────────────────────────────────────────────────────
POST {{base_url}}/api/recent/folder/{{test1_id}}
Authorization: Bearer {{token}}

HTTP 200


# ─────────────────────────────────────────────────────────────
# Step 8 – Recent contains both items, most recent first (folder)
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/api/recent
Authorization: Bearer {{token}}

HTTP 200
[Asserts]
jsonpath "$" count == 2
jsonpath "$[0].item_id" == {{test1_id}}
jsonpath "$[0].item_type" == "folder"
jsonpath "$[1].item_id" == {{file_id}}
jsonpath "$[1].item_type" == "file"


# ─────────────────────────────────────────────────────────────
# Step 9 – Remove file from recent
# ─────────────────────────────────────────────────────────────
DELETE {{base_url}}/api/recent/file/{{file_id}}
Authorization: Bearer {{token}}

HTTP 200


# ─────────────────────────────────────────────────────────────
# Step 10 – Recent contains only folder test1
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/api/recent
Authorization: Bearer {{token}}

HTTP 200
[Asserts]
jsonpath "$" count == 1
jsonpath "$[0].item_id" == {{test1_id}}


# ─────────────────────────────────────────────────────────────
# Step 11 – Clear all recent items
# ─────────────────────────────────────────────────────────────
DELETE {{base_url}}/api/recent/clear
Authorization: Bearer {{token}}

HTTP 200


# ─────────────────────────────────────────────────────────────
# Step 12 – Recent is empty again
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/api/recent
Authorization: Bearer {{token}}

HTTP 200
[Asserts]
jsonpath "$" count == 0
