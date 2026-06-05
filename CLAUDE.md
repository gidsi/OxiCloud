# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# Architecture

This project is split into two parts:
- `/src` — OxiCloud Backend server in **Rust**
- `/static` — Oxicloud Frontend in **vanilla CSS & vanilla JavaScript**

# Backend part

## Backend Build & Dev Commands

```bash
cargo build                          # Dev build
cargo build --release                # Optimized release build
cargo run                            # Run server (port 8086)
cargo test --workspace               # Run all tests (~208)
cargo test <test_name>               # Run a single test by name
cargo test --features test_utils     # Run tests that use mockall mocks
cargo clippy -- -D warnings          # Lint (zero warnings policy)
cargo fmt --all --check              # Format check
cargo fmt --all                      # Auto-format
RUST_LOG=debug cargo run             # Run with debug logging
cargo run --bin generate-openapi     # Regenerate resources/gen/openapi.json
```

A `justfile` is available for common tasks (`just --list` to see all). Key recipes: `just check` (fmt + clippy), `just test`, `just openapi`.

Requires **Rust 1.93+** (edition 2024) and **PostgreSQL 13+** (with `pg_trgm` and `ltree` extensions).

Database setup: `docker compose up -d postgres` — schema is applied automatically via sqlx migrations on app startup. Migration files live in `migrations/`. For local dev, set `DATABASE_URL` in `.env` (see `example.env`).

## Backend Pre-commit checks

Always run these before committing, in this order:

```bash
cargo fmt --all                                              # Auto-format
cargo clippy --all-features --all-targets -- -D warnings     # Lint (must pass with zero warnings)
```

CI enforces both — commits that fail either check will not merge.

## Backend Architecture

Hexagonal / Clean Architecture with four layers. Dependencies point inward only.

### Layer structure (`src/`)

- **`domain/`** — Core business entities (`entities/`) and repository trait definitions (`repositories/`). Pure Rust, no framework dependencies. Entity types: `File`, `Folder`, `User`, `Calendar`, `CalendarEvent`, `Contact`, `Share`, `TrashedItem`, `Session`, `DeviceCode`, `AppPassword`.

- **`application/`** — Use cases and orchestration.
  - `ports/` — Trait definitions (inbound/outbound) for storage, auth, caching, compression, dedup, thumbnails, chunked uploads, CalDAV/CardDAV, etc. This is the hexagonal "ports" layer.
  - `services/` — Use case implementations (`FileManagementService`, `FolderService`, `ShareService`, `TrashService`, `CalendarService`, `ContactService`, `SearchService`, `BatchOperations`, etc.).
  - `adapters/` — CalDAV/CardDAV protocol adapters (iCalendar/vCard parsing).
  - `dtos/` — Data transfer objects for API boundaries.

- **`infrastructure/`** — Concrete implementations of ports.
  - `repositories/pg/` — All PostgreSQL repository implementations (via `sqlx`). Uses `auth` schema for users/sessions, `storage` schema for files/folders/blobs (content-addressable dedup with ltree paths).
  - `services/` — JWT, password hashing (Argon2), OIDC, compression, thumbnails, chunked uploads, WOPI discovery, WebDAV locking, file content caching (moka).
  - `adapters/` — CalDAV/CardDAV storage adapters bridging domain traits to PG.
  - `db.rs` — Dual connection pool setup (user pool + maintenance pool).

- **`interfaces/`** — HTTP layer (Axum).
  - `api/handlers/` — REST API handlers for files, folders, auth, admin, search, shares, WebDAV, CalDAV, CardDAV, WOPI, chunked uploads, batch operations.
  - `api/routes.rs` — Route registration, splits protected vs public routes.
  - `nextcloud/` — NextCloud-compatible API (WebDAV, OCS, login flow v2, trashbin) with Basic Auth middleware.
  - `middleware/` — Auth (JWT validation), CSRF, rate limiting.
  - `web/` — Static file serving.

- **`common/`** — Cross-cutting concerns.
  - `di.rs` — `AppServiceFactory` builds all services and produces `AppState` (the central DI container passed to Axum). This is the composition root.
  - `config.rs` — `AppConfig::from_env()` loads all `OXICLOUD_*` env vars.

### Key patterns

- **DI via `AppState`**: All services are `Arc`-wrapped and assembled in `common/di.rs`. `AppState` is wrapped in `Arc` and passed as Axum state. Many services are `Option<Arc<T>>` because they depend on features being enabled (auth, WOPI, trash, etc.).

- **Content-addressable storage**: Files use BLAKE3 blob dedup. `storage.file_blobs` stores content; `storage.file_metadata` references blobs with ref-counting. See `file_blob_write_repository.rs` and `file_blob_read_repository.rs`.

- **ltree paths**: Folder hierarchy uses PostgreSQL `ltree` for efficient subtree queries (recursive copies, moves, searches).

- **Dual DB pools**: `DbPools` in `infrastructure/db.rs` separates user-facing queries from maintenance/background tasks to prevent starvation.

- **Feature flags**: Major features (auth, trash, search, sharing, quotas) are toggled via `OXICLOUD_ENABLE_*` env vars in `FeaturesConfig`.

- **UUID columns**: All ID columns use native PostgreSQL `UUID` type. SQL queries must use `::uuid` casts when passing string parameters to UUID columns.

### Database schemas

- `auth` schema: `users`, `sessions`, `app_passwords`, `device_codes`, `admin_settings`
- `storage` schema: `folders`, `file_metadata`, `file_blobs`, `trash`, `shares`, `favorites`, `recent_items`, `nextcloud_object_ids`
- `caldav` schema: `calendars`, `calendar_events`
- `carddav` schema: `address_books`, `contacts`, `contact_groups`, `contact_group_members`

Schema definition: `migrations/` (sqlx migrations, applied on startup)

### Protocol support

The server exposes multiple protocol interfaces simultaneously:
- REST API under `/api/`
- WebDAV at `/webdav/` (RFC 4918)
- CalDAV at `/caldav/`
- CardDAV at `/carddav/`
- NextCloud-compatible API at `/remote.php/`, `/ocs/`, `/status.php`
- WOPI at `/wopi/` (when enabled)
- Well-known discovery at `/.well-known/caldav` and `/.well-known/carddav`

### Test organization

Tests are primarily `#[cfg(test)]` modules within source files (~36 files have inline tests). Dedicated test files exist at `*_test.rs` alongside their source. The `test_utils` feature flag enables `mockall` mock generation for trait-heavy testing. No separate `tests/` directory.

### Code duplication

Never duplicate logic across handlers or services. If the same behaviour is needed in more than one place, extract it into a shared function, method, or service before writing the second callsite. Preferred homes by layer:
- Cross-handler request logic → method on `CoreServices` or `AppState` (`common/di.rs`)
- Reusable infrastructure behaviour → method on the relevant service struct
- Shared port behaviour → default method on the trait

### Authorization (AuthZ)

**AuthZ is enforced exclusively in the application service layer, never in handlers.** All permission checks go through `AuthorizationEngine` (port: `application/ports/authorization_ports.rs`) via service methods named with the `_with_perms` suffix. HTTP handlers (REST, WebDAV, NextCloud, CalDAV, CardDAV) authenticate the caller and pass `caller_id` into the service — they MUST NOT perform their own ownership/permission checks. The authentication middleware extracts the caller; the service decides if the action is allowed.

This rule prevents drift between layers and ensures every code path goes through the same policy. New service methods that touch a user-scoped resource must take `caller_id: Uuid` and call `authz.require(...)` before any read or mutation.

### Audit logging for denials and rejections

**Every permission denial or auth rejection MUST emit a structured audit log line before returning the error.** Without one, security-relevant outcomes are invisible to operators and incident response loses its primary signal.

The convention:

```rust
tracing::info!(
    target: "audit",
    event = "<domain>.<outcome>",     // e.g. "authz.denied", "auth.login_rejected",
                                       //      "magic_link.redemption_rejected",
                                       //      "user_profile.rejected"
    reason = "<short_key>",            // stable machine-readable key for filtering
                                       // (e.g. "bad_password", "expired", "no_visibility_path")
    // …structured fields naming the actors / targets…
    caller_id = %caller_id,            // or subject_id, user_id, granted_by, etc.
    target_id = %target_id,            // or resource_id, subject_id, etc.
    "👮🏻‍♂️ human-readable message: …",  // helpful for live tailing, do not parse
);
```

Rules:

- **`target: "audit"`** routes the line to the audit channel (separable from operational `oxicloud::*` debug noise).
- **`event`** uses the dotted form `<domain>.<verb_past_tense>` and stays stable — log aggregators key off it.
- **`reason`** is a machine-readable enum-style key. Don't reword across releases. New denial cause → new `reason` value, never repurpose an existing one.
- **Structured fields** carry every actor/target involved (`caller_id`, `target_id`, `resource_id`, `subject_id`, role, is_external flag, etc.). Request id and client IP come from the request-scope span automatically — don't duplicate them.
- **Anti-enumeration is preserved.** Returning `NotFound` to the caller while logging the real reason internally is the canonical pattern (e.g. `user_profile.rejected` with `reason = "external_caller_no_relationship"` returns 404, never 403). Operators see the truth; the attacker sees the same response shape regardless of whether the user exists.
- **Success paths stay quiet** by default — every authorized request would otherwise flood the log. Use `tracing::debug!` with `target: "oxicloud::authz"` (or similar) when a low-volume granted-trace helps debugging. Reserve `tracing::info!(target: "audit", …)` for outcomes worth surfacing in security reviews.

Canonical examples to mirror: `authz.denied` in `application/ports/authorization_ports.rs::require`, `auth.login_rejected` and `magic_link.redemption_rejected` and `user_profile.rejected` in `application/services/auth_application_service.rs`.

# Frontend part

## Code conventions

### Javascript

- ES Modules (import/export), no CommonJS
- No frameworks — vanilla JS only
- Naming: `camelCase` for variables/functions, `PascalCase` for classes
- No `var` — use `const`/`let` only
- **JSDoc required** on all public functions — `jsconfig.json` enables `checkJs` globally (equivalent to `@ts-check` on every file)
- Always us static/js/core/types.js to mapp OxCcloud API structure
- Type parameters, return types, and complex types via `@typedef`:

```js
/**
 * @typedef {Object} User
 * @property {number} id
 * @property {string} name
 */

/**
 * @param {User} user
 * @param {string} [role="viewer"]
 * @returns {Promise}
 */
async function updateUser(user, role = 'viewer') { … }
```

### Code duplication

Never duplicate logic across JS modules. If the same behaviour is needed in more than one place, extract it into a shared utility function and import it. Preferred homes:
- DOM/UI helpers → `static/js/utils/` or an existing utility module
- API call wrappers → the relevant API module (e.g. `api/files.js`)
- Event or state patterns shared across components → a dedicated shared module

### CSS
- BEM methodology for class names (`.block__element--modifier`)
- CSS custom properties in `:root` for colors and spacing
- **All colors must use `var(--*)` — no raw hex, rgb, or named colors anywhere except in `:root` declarations**
- Mobile-first: media queries expand, they don't restrict
- One CSS file per logical component in `/static/css/`
- [data-theme="dark"] is permitted only in /static/css/themes/dark.css

## Frontend Pre-commit checks

Always run these before committing, in this order:

```bash
biome check --fix                                           # Auto-format
biome lint  --fix                                           # Lint (must pass)
stylelint static/css/                                       # Css rules
tsc -p jsconfig.json --noEmit                               # Ensure JS is always typed
```

# What Claude must NOT do
- Edit `Cargo.lock` directly
- Use npm dependencies not listed in this file
- Introduce a JS framework (React, Vue, etc.) without explicit approval
- Leave debug `console.log` statements in code
- Use raw color values in CSS — always use CSS custom properties
- Commit without passing all linters
