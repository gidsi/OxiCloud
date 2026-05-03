# DAV Compliance Tests

Black-box protocol compliance testing for OxiCloud's WebDAV, CalDAV, and CardDAV endpoints.

## What runs in CI

| Suite | Protocols | Source |
|-------|-----------|--------|
| **litmus** | WebDAV (RFC 2518 / 4918) | [webdav.org/neon/litmus](http://www.webdav.org/neon/litmus/) |
| **CalDAVTester** | CalDAV (RFC 4791) + CardDAV (RFC 6352) | [apple/ccs-caldavtester](https://github.com/apple/ccs-caldavtester) |

## Running locally

### Prerequisites

```bash
# litmus
sudo apt-get install litmus

# CalDAVTester
git clone https://github.com/apple/ccs-caldavtester.git
git clone https://github.com/niccokunzmann/ccs-pycalendar.git
ln -s ../ccs-pycalendar ccs-caldavtester/pycalendar
```

### Start OxiCloud

```bash
docker compose up -d postgres
cargo run &
# Wait for http://localhost:8086/version to respond
```

### Provision test users

```bash
# Register
curl -X POST http://localhost:8086/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"user1","email":"user1@test.local","password":"TestPass123!"}'

# Login → get JWT
TOKEN=$(curl -s -X POST http://localhost:8086/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"user1","password":"TestPass123!"}' | jq -r '.token // .access_token')

# Create app password for DAV auth
APP_PASS=$(curl -s -X POST http://localhost:8086/api/auth/app-passwords \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"name":"local-test"}' | jq -r '.password // .app_password')
```

### Run litmus

```bash
litmus http://localhost:8086/webdav/user1/ user1 "$APP_PASS"
```

### Run CalDAVTester

```bash
# Substitute passwords into serverinfo.xml
sed -e "s|__USER1_APP_PASS__|$APP_PASS|g" \
    -e "s|__USER2_APP_PASS__|$APP_PASS2|g" \
    tests/compliance/serverinfo.xml > ccs-caldavtester/serverinfo-oxicloud.xml

cd ccs-caldavtester
python testcaldav.py --print-details-onfail \
  --serverinfo serverinfo-oxicloud.xml -s CalDAV
```

## Current status

🚧 **Baseline establishment** — the CI pipeline currently allows failures (`|| true`)
to collect a compliance baseline. Once the critical tests pass, the `|| true` guards
will be removed to enforce compliance on every PR.
