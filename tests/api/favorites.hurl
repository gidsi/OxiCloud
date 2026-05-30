# =============================================================
# OxiCloud – Favorites API end-to-end scenario
# =============================================================
# Run:
#   hurl --variables-file tests/api/test.env --test tests/api/favorites.hurl
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
# Step 2 – No favorites yet
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/api/favorites
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
# Step 4 – Add hello.txt to favorites
# ─────────────────────────────────────────────────────────────
POST {{base_url}}/api/favorites/file/{{file_id}}
Authorization: Bearer {{token}}

HTTP 201


# ─────────────────────────────────────────────────────────────
# Step 5 – Favorites contains only hello.txt
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/api/favorites
Authorization: Bearer {{token}}

HTTP 200
[Asserts]
jsonpath "$" count == 1
jsonpath "$[0].item_id" == {{file_id}}
jsonpath "$[0].item_type" == "file"
jsonpath "$[0].item_name" == "hello.txt"


# ─────────────────────────────────────────────────────────────
# Step 6 – Add test1 folder to favorites
# ─────────────────────────────────────────────────────────────
POST {{base_url}}/api/favorites/folder/{{test1_id}}
Authorization: Bearer {{token}}

HTTP 201


# ─────────────────────────────────────────────────────────────
# Step 7 – Favorites contains both items (order-independent)
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/api/favorites
Authorization: Bearer {{token}}

HTTP 200
[Asserts]
jsonpath "$" count == 2
jsonpath "$[*].item_id" contains {{file_id}}
jsonpath "$[*].item_id" contains {{test1_id}}


# ─────────────────────────────────────────────────────────────
# Step 8 – Remove hello.txt from favorites
# ─────────────────────────────────────────────────────────────
DELETE {{base_url}}/api/favorites/file/{{file_id}}
Authorization: Bearer {{token}}

HTTP 200


# ─────────────────────────────────────────────────────────────
# Step 9 – Favorites contains only test1 folder
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/api/favorites
Authorization: Bearer {{token}}

HTTP 200
[Asserts]
jsonpath "$" count == 1
jsonpath "$[0].item_id" == {{test1_id}}
jsonpath "$[0].item_type" == "folder"
jsonpath "$[0].item_name" == "test1"


# ─────────────────────────────────────────────────────────────
# Step 10 – Cleanup: remove test1 from favorites
# ─────────────────────────────────────────────────────────────
DELETE {{base_url}}/api/favorites/folder/{{test1_id}}
Authorization: Bearer {{token}}

HTTP 200


# ─────────────────────────────────────────────────────────────
# Step 11 – Favorites is empty again
# ─────────────────────────────────────────────────────────────
GET {{base_url}}/api/favorites
Authorization: Bearer {{token}}

HTTP 200
[Asserts]
jsonpath "$" count == 0
