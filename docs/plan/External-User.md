# Plan — Magic-link external authentication

## Context

The UserLifecycleHook plan (PRs 1-5) shipped: `is_external` flag, `User::new_external`, lifecycle dispatcher with five hooks, `ExternalIdentityLifecycleHook` registered as a no-op stub awaiting this work. The DB CHECK `users_external_no_storage` and `users_external_not_admin` are in place. The `auth.users` table can already hold external recipients; nothing addresses them yet.

This plan implements the recipient-side flow: an internal user shares a resource by email; the server resolves the email to an existing user OR creates an external user on the fly; an invitation email is sent; the recipient clicks the magic link and lands on the resource (deep link) or on `/shared-with-me` (generic email login). External users have no password and authenticate exclusively via magic link until they later set a credential (password / OIDC / future webauthn), at which point magic-link silently becomes unavailable for that account.

The end state: OxiCloud can share with people who don't have accounts yet, with the same authz semantics as any other grant; the sharer cannot enumerate who already has an account (uniform API response shape); admin holds a kill switch (`OXICLOUD_ALLOW_EXTERNAL_USERS=false`).

## Design decisions (locked in)

### Security model — "Option A, nuanced"

A user is **magic-link-eligible** iff they have no other authentication method configured. Encapsulated in:

```rust
impl User {
    pub fn has_login_credential(&self) -> bool {
        self.password_hash != "__EXTERNAL_NO_PASSWORD__"
            && self.password_hash != "__OIDC_NO_PASSWORD__"
            || self.oidc_subject.is_some()
    }
}
```

The placeholder-string approach is a known smell — a proper `auth.user_auth_methods` side-table is the future evolution path, listed in "Future work" below. Today every magic-link-eligibility check goes through `has_login_credential()`, so the migration to the side-table only touches that method's body.

State graph (verified by `has_login_credential()`):

| State | password_hash | oidc_subject | Magic-link eligible |
|---|---|---|---|
| External, freshly invited | `__EXTERNAL_NO_PASSWORD__` | NULL | yes |
| External who set password | real argon2 hash | NULL | no |
| External who linked OIDC | `__OIDC_NO_PASSWORD__` | set | no |
| Internal, password | real argon2 hash | NULL | no |
| Internal, OIDC-only | `__OIDC_NO_PASSWORD__` | set | no |

Internal users who receive a "Bob shared FILE with you" mail get a notification-only link that deep-links to the OxiCloud login page with a return URL — no auto-auth, no mailbox-as-2FA-bypass.

### Identity: username = email for external users

- External users get `username = normalized_email`.
- `auth.users.username` length cap widened from 32 to 254 (RFC 5321 maximum).
- Login form accepts username OR email; lookup tries `username` first, falls back to `email`.
- Username becomes mutable (post-create), via a new endpoint. The home folder name (`"My Folder - alice"`) is **not** renamed when username changes — it was display text at creation; semantically the folder is owned by `user_id`.
- New columns `auth.users.given_name` and `auth.users.family_name`, both `TEXT NULL` — populated from OIDC standard claims at JIT provisioning; external users get NULL initially; users can set them later via a profile-edit endpoint.

### Email normalization

```rust
fn normalize_email(input: &str) -> Result<String, ValidationError> {
    let trimmed = input.trim();
    let (local, domain) = trimmed.rsplit_once('@').ok_or(Malformed)?;
    let local_lower = local.to_lowercase();
    let domain_ascii = idna::domain_to_ascii(&domain.to_lowercase())
        .map_err(|_| InvalidDomain)?;
    Ok(format!("{}@{}", local_lower, domain_ascii))
}
```

Stored form is always ASCII (punycode for IDN domains). UI can reverse for display via `idna::domain_to_unicode`. Local-part case-folding to lower; Gmail `+tag` and `.` insensitivity are NOT special-cased (treat strings as opaque post-normalization).

### Internal virtual group finally narrowed

`pg_acl_engine.rs::expand_user` today inserts `INTERNAL_GROUP_ID` unconditionally with a TODO: *"Once the external-users work lands this will narrow to `if !user.is_external { ... }`."* Now's the time. External users do NOT belong to the Internal virtual group. The group's name finally honours its semantics.

### Magic-link tokens — mirror `auth.device_codes`

The closest existing pattern is `auth.device_codes` (entity at `src/domain/entities/device_code.rs`, repo at `src/infrastructure/repositories/pg/device_code_pg_repository.rs`). Status enum with PostgreSQL custom type, plain-text token, indexed on `expires_at WHERE pending`, `delete_expired()` cleanup helper. Copy verbatim.

New table:

```sql
CREATE TYPE auth.magic_link_status AS ENUM ('pending', 'used', 'expired');

CREATE TABLE auth.magic_link_tokens (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token          TEXT NOT NULL UNIQUE,    -- 32 random bytes, base64url
    user_id        UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    status         auth.magic_link_status NOT NULL DEFAULT 'pending',
    issued_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at     TIMESTAMPTZ NOT NULL,
    used_at        TIMESTAMPTZ,
    -- Optional deep-link target. NULL → generic "login via email" flow,
    -- lands on /shared-with-me. NOT NULL → invitation, lands directly.
    resource_type  TEXT CHECK (resource_type IN ('file', 'folder')),
    resource_id    UUID,
    CHECK ((resource_type IS NULL) = (resource_id IS NULL))
);

CREATE INDEX ON auth.magic_link_tokens (expires_at) WHERE status = 'pending';
CREATE INDEX ON auth.magic_link_tokens (user_id, status);
```

Token lifetime: env-driven `OXICLOUD_MAGIC_LINK_TTL_HOURS` (default 24).

### Sharing flow extends `POST /api/grants`

New shape for the request body's `subject`:

```json
{
  "subject":   { "type": "email", "email": "Alice@Example.COM" },
  "resource":  { "type": "folder", "id": "..." },
  "role":      "viewer",
  "expires_at": "...",
  "notify":    true,
  "message":   "Hi Alice, here's the report."
}
```

Server flow (uniform response shape to defeat enumeration):

1. Validate `email` regex.
2. Normalize (lowercase + punycode).
3. Look up by normalized email (case-insensitive query against `auth.users.email`).
4. If found: use existing `user_id`. If not: respect `OXICLOUD_ALLOW_EXTERNAL_USERS`. If false: 403. Else: `User::new_external(email_as_username, email)` and `dispatch_created`.
5. Build grant with `subject = User(uuid)`.
6. If `notify` (required `true` in v1): issue magic-link token targeting the resource, send email via `EmailSender` port.
7. Return standard `GrantDto` with the resolved `user_id`.

Latency is the enumeration risk: existing user is a single SELECT; new user is SELECT + INSERT + INSERT + SMTP. The SMTP send goes through `tokio::spawn` (fire-and-forget) so the API response timing doesn't differ meaningfully between the two paths. Server logs the SMTP failure (if any) but the response stays uniform.

`notify = false` is rejected with 400 in v1 (no way for the recipient to access otherwise). Reserved for future "I'll send the URL myself via Slack" flow.

### Landing UX

```
Magic link in invitation mail (resource_type/id NOT NULL)
  ↓
/magic/v1/{token}
  ↓ (validate, mark used, emit session)
  ↓
Redirect to /folders/{id} or /files/{id} — direct to the resource
```

```
"Login via email" form on /login (user types their email)
  ↓
POST /api/auth/magic-link/send  (uniform response)
  ↓ (if user has no credential, issue token with NULL resource, send mail)
  ↓
User clicks /magic/v1/{token}
  ↓ (validate, mark used, emit session)
  ↓
Redirect to /shared-with-me — their home for incoming grants
```

Same redemption endpoint, different landing logic keyed on whether the token has a resource target.

### Configuration

```
OXICLOUD_SMTP_HOST=smtp.example.com
OXICLOUD_SMTP_PORT=587
OXICLOUD_SMTP_USER=oxicloud@example.com
OXICLOUD_SMTP_PASS=...
OXICLOUD_SMTP_FROM="OxiCloud <noreply@example.com>"
OXICLOUD_SMTP_TLS=starttls       # starttls | tls | none
OXICLOUD_MAGIC_LINK_TTL_HOURS=24
OXICLOUD_ALLOW_EXTERNAL_USERS=true   # set false to disable the whole feature
OXICLOUD_PUBLIC_URL=https://oxicloud.example.com  # for building link URLs
```

`EmailSender` is `Option<Arc<dyn EmailSender>>` in DI — `None` when SMTP isn't configured. Endpoints that require email return 503 in that state with a clear "SMTP not configured" message.

### Rate limits

Reusing the existing `RateLimiter` at `src/interfaces/middleware/rate_limit.rs` (moka cache + counter, sliding window). Two new limiters:

- **Per-sharer email invitation**: 50 / hour, keyed by `caller_id`. Defends against an admin or compromised account spamming invites.
- **Per-target-email resend**: 5 / hour, keyed by the normalized email being resent to. Defends against the resend endpoint being used as an email-bombing primitive.

### Defense in depth — boundary protections for external users

External users are a new principal kind. Several existing surfaces implicitly assume "all users are internal employees of this instance" and would leak / over-share once externals show up. **PR 6 closes all of these gaps** (alongside the schema groundwork) so subsequent PRs in this sequence don't accidentally surface external users where they don't belong.

**Already protected (by PR 2 of the lifecycle work — verified)**:

- DB CHECK `users_external_not_admin`: an external user cannot hold admin role. Three-layer enforcement (DB + entity factory + handler).
- DB CHECK `users_external_no_storage`: an external user's `storage_used_bytes` must always be 0.
- `HomeFolderLifecycleHook::provision_if_needed` short-circuits on `user.is_external()` — no home folder for externals.
- `INTERNAL_GROUP_ID` is immutable (membership is implicit, additions/removals rejected as `VirtualImmutable` at the service layer).

**Already-existing gaps this work must close (PR 6)**:

1. **Subject groups admit external users today.** `subject_group_service.rs::add_member` (line 238) protects the `Internal` virtual group but does **not** reject `GroupMember::User(uuid)` where the candidate has `is_external = TRUE`. Concrete attack: admin adds `alice@example.com` (external) to the "Engineering" group; "Engineering" later gets a grant on internal-only resources; alice silently gains access. **Fix**: in `add_member`, after the `INTERNAL_GROUP_ID` guard, fetch the candidate user and reject with `DomainError::AccessDenied` if `user.is_external()` is true. Error message: "External users cannot be members of subject groups; share resources with them directly." Mirrors the no-external-admins enforcement style.

2. **System-contacts endpoint surfaces every user.** `contacts_handler::list_contacts(book_id=SYSTEM_BOOK_ID)` (line 447) calls `auth_service.list_users` which returns all users including externals. The share modal autocomplete (via `addressBook.searchContacts(q, [SYSTEM_BOOK_ID])`) would then suggest external users as recipients — wrong UX, and also leaks external identities to other internal users. **Fix**: `auth_service.list_users` and `auth_service.search_users` accept an `include_external: bool` parameter, defaulting to `false`. SQL adds `WHERE is_external = FALSE` when the flag is off. Existing call sites pass `false`. A new admin-list-users endpoint can pass `true` if the admin UI ever needs to show externals (handled in a future PR; not in scope here).

3. **`expand_user` adds external users to `INTERNAL_GROUP_ID`.** The TODO in `pg_acl_engine.rs:141` (*"narrow to if !user.is_external"*). **Fix**: include the conditional. External users get an expansion of `{their_uid}` only, no implicit Internal membership. This protects every Internal-group grant from inadvertent leakage to externals.

**Considered and intentionally deferred to a future hardening PR** (documented in "Out of scope"):

- **External users with `Permission::Share` resharing to create more externals.** Today nothing stops an external `Share`-grantee from invoking the email-grant flow and minting new external users. Policy question: should we forbid externals from being a `granted_by` value? Possible env flag: `OXICLOUD_EXTERNAL_USERS_CAN_RESHARE=false`. Not in this work.
- **Shorter session/refresh-token TTL for external users.** Today refresh-token expiry is global. The plan keeps it that way for v1; future env `OXICLOUD_EXTERNAL_REFRESH_TOKEN_EXPIRY_DAYS` for differentiated lifetimes.
- **`session_kind` tagging on sessions emitted from magic-link.** Could enable scoped sessions later (Option B from the security discussion). Not in v1.

**Magic-link-specific protections built into PR 8/9**:

- Tokens are 32-byte random base64url, generated via the OS CSPRNG (same pattern as `device_codes`).
- Single-use via `status = 'used'` + `used_at` stamp; second redemption attempt rejected with 400 ("link already used").
- TTL-enforced at the redemption endpoint (`expires_at < NOW()` → 400 "link expired").
- Redemption endpoint is `GET /magic/v1/{token}` (token in URL path, not query string, to keep it out of `Referer` headers). On successful redemption the server immediately 302s to the resource — the magic-link URL is replaced in the address bar before the user can navigate further.
- Uniform response on `POST /api/auth/magic-link/send` (`"If we have an account, a link will be sent"`) regardless of whether the email exists. Per-target-email rate limit prevents using the endpoint as an enumeration oracle by latency.

## PR sequence

| PR | Subject | Why land separately |
|---|---|---|
| **6** | Prelude: `has_login_credential()`, narrow `INTERNAL_GROUP_ID`, widen username, add `given_name`/`family_name`, make username mutable, **+ defense-in-depth: subject groups reject external members, `list_users`/`search_users` filter externals, `expand_user` excludes externals from Internal** | Entity + schema groundwork **plus the three boundary protections enumerated in "Defense in depth"**. Verifiable in isolation by running the existing Hurl suite — no new behaviour for internal users, just protections that activate once externals exist. |
| **7** | SMTP infrastructure: `EmailSender` port + `lettre`-backed impl + env config | Pure infrastructure. Mocked in tests. No user-visible feature yet. |
| **8** | `auth.magic_link_tokens` table + repo + redemption endpoint `/magic/v1/{token}` + `ExternalIdentityLifecycleHook` populated | The magic-link plumbing. Tokens can be manually fabricated for unit tests; sharer flow still pending. |
| **9** | Extend `POST /api/grants` for `subject.type = "email"` + email normalization + lazy external-user creation + invitation email + Hurl coverage of the invite path | The sharer side, end-to-end. The Hurl test creates an unknown email, claims the resulting magic link, lands on the resource. |
| **10** | Login-via-email endpoint (`POST /api/auth/magic-link/send`) with uniform response + landing on `/shared-with-me` for NULL-resource tokens | The recovery / no-password-yet path. Lands the existing user back into their incoming-grants view. |
| **11** | Frontend: share-modal accepts arbitrary email + login page "Login with email link" section | UI changes alone. Pure frontend PR for clean review. |
| **12** | Rate limits + comprehensive Hurl coverage + architecture doc + sidebar | Hardening + acceptance gate. `docs/architecture/magic-link-auth.md` + sidebar entry. Updated `share-integration.md`. |

## Critical files

**New files**:

- PR 6: `migrations/20260612000003_users_username_email_login.sql` (widen username, add given_name/family_name, mutable username)
- PR 7: `Cargo.toml` (+lettre), `src/application/ports/email_sender.rs`, `src/infrastructure/services/smtp_email_sender.rs`
- PR 8: `migrations/20260612000004_magic_link_tokens.sql`, `src/domain/entities/magic_link_token.rs`, `src/infrastructure/repositories/pg/magic_link_token_pg_repository.rs`, `src/interfaces/api/handlers/magic_link_handler.rs`
- PR 9: `src/domain/services/email_normalize.rs` (small utility), invitation email template inline in `external_identity_service.rs`
- PR 12: `docs/architecture/magic-link-auth.md`

**Modified files**:

- PR 6: `src/domain/entities/user.rs` (`has_login_credential`, username mutability getter/setter), `src/infrastructure/services/pg_acl_engine.rs` (drop the unconditional `INTERNAL_GROUP_ID` insert when `user.is_external()` — closes protection gap #3), `src/application/services/auth_application_service.rs` (login lookup tries email fallback; `list_users` / `search_users` gain `include_external: bool` defaulting to false — closes protection gap #2), `src/application/services/subject_group_service.rs` (`add_member` rejects external user members — closes protection gap #1), `src/application/dtos/user_dto.rs` (given_name/family_name fields), `src/infrastructure/repositories/pg/user_pg_repository.rs` (`list_users` / `search_users` SQL gains `WHERE is_external = FALSE` when filter is on)
- PR 7: `src/common/di.rs` (wire `EmailSender`), `src/common/config.rs` (parse SMTP env vars)
- PR 8: `src/application/services/external_identity_service.rs` (populate the PR-5 stub), `src/common/di.rs` (wire magic_link_repo into external_identity hook)
- PR 9: `src/interfaces/api/handlers/grant_handler.rs` (extend `POST /api/grants` request parsing), `src/application/dtos/grant_dto.rs` (new SubjectTypeDto variant; or accept email-as-string in existing SubjectDto), `src/interfaces/api/routes.rs`
- PR 10: `src/interfaces/api/routes.rs` (register `/api/auth/magic-link/send`), `src/application/services/auth_application_service.rs` (login-via-email use case)
- PR 11: `static/js/components/shareModal.js` (free-text email input), `static/login.html` (new section), `static/js/features/auth/auth.js` (POST flow + success UI), i18n keys in 16 locales
- PR 12: `src/interfaces/middleware/rate_limit.rs` (two new limiter constructors), `tests/api/magic_link.hurl` (new test file), `docs/.vitepress/config.mts` (sidebar entry), `docs/architecture/share-integration.md` (cross-reference)

## Existing patterns to reuse (with paths)

- **Rate limiter**: `src/interfaces/middleware/rate_limit.rs` — `RateLimiter::new(max_requests, window_secs, max_entries)` + `check_and_increment(&key)`. Two new factory functions (`rate_limit_email_invite`, `rate_limit_magic_link_send`).
- **Token storage pattern**: `src/domain/entities/device_code.rs` + `src/infrastructure/repositories/pg/device_code_pg_repository.rs`. Status enum (pending/used/expired) with PostgreSQL custom type; `delete_expired()` cleanup helper.
- **Lifecycle hook**: `ExternalIdentityLifecycleHook` already registered in DI (PR 5). Body filled in here.
- **Audit pattern**: `tracing::info!(target: "audit", event = "...")` — same convention as `subject_group_service.rs` and `user_lifecycle_service.rs`.
- **Email-input UX in share modal**: Today autocomplete-only (lines 350-391 of `shareModal.js`). Add a third "external email" suggestion type alongside `ContactItem` and `GroupSuggestion` — uses the same staging/chip rendering machinery.
- **Login page extensibility**: `static/login.html` lines 59-121 + `static/js/features/auth/auth.js::initLoginElements` lines 758-810. New section mirrors the OIDC button pattern.
- **Idna for punycode**: add `idna` crate to Cargo.toml; standard Rust crate for IDN handling.

## Verification

Per-PR (all PRs):

```bash
cargo fmt --all
cargo clippy --all-features --all-targets -- -D warnings
cargo test --workspace
bash tests/api/run.sh        # 13 existing Hurl files must still pass
```

End-to-end gate after PR 9:

1. Login as admin in browser. Share a folder with `newly-invited@example.com`. Confirm:
   - HTTP 201 + grant_id returned.
   - `auth.users` has a new row, `is_external = TRUE`, `username = 'newly-invited@example.com'`, no password_hash (placeholder).
   - `auth.magic_link_tokens` has a new row pointing at that user and the folder.
   - SMTP relay receives one mail (use MailHog or `OXICLOUD_SMTP_HOST=localhost` + a netcat trap).
2. Open the magic-link URL from the captured mail. Confirm:
   - Session cookie issued; redirected to the folder URL.
   - Token row's `status = 'used'`, `used_at` set.
3. Reload the URL. Confirm 400 "link already used".
4. Wait past TTL on a fresh token; confirm 400 "link expired" + "Resend" UI.

End-to-end gate after PR 10:

5. Log out. Go to `/login`. Click "Login with email link". Enter the same email. Confirm:
   - HTTP 200 with uniform "If we have an account, a link will be sent" body.
   - Fresh magic-link token in DB (no resource target this time).
   - Mail received. Click → land on `/shared-with-me`. Confirm the previously shared folder is in the list.

End-to-end gate after PR 12:

6. Issue 60 invitations from one admin in a minute → confirm 50 succeed and 10 are rate-limited with 429.
7. POST `/api/auth/magic-link/send` 10× for the same email in 10 minutes → confirm 5 succeed and 5 are rate-limited with 429.
8. Hurl suite `tests/api/magic_link.hurl` covers: invite-new-email, invite-existing-email (no duplicate user), token redemption, expired token, resend uniform response, rate-limit triggers.

## Out of scope (do NOT bundle)

- **Auth-method side-table refactor.** Acknowledged smell with the placeholder strings (`__EXTERNAL_NO_PASSWORD__` etc.). Future PR introduces `auth.user_auth_methods` with rows per `(user_id, method_type, credentials)`. The `has_login_credential()` method is the single migration point; refactor changes its body without rippling.
- **Email template engine + i18n localization of emails.** v1 ships English-only hardcoded templates. Template engine (handlebars / askama) + recipient-locale detection is a future PR.
- **MX-record validation at share time.** Regex only; bad domains discover themselves via SMTP bounce.
- **Periodic cleanup of dormant external users.** A sweeper that purges users with no `last_login_at` for 13+ months. Future PR; the GDPR-sweeper variant `DeletionMode::GdprPurge` (already in the trait) is its hook entry point.
- **Per-instance allowlist of external email domains** (e.g. only `*@my-company.com`). Future env var `OXICLOUD_EXTERNAL_EMAIL_DOMAINS`. Kill switch (`OXICLOUD_ALLOW_EXTERNAL_USERS=false`) ships in PR 6 as a coarser tool.
- **WebAuthn / passkey enrolment for external users after first login.** Distinct future feature; the magic-link bootstrap is the prerequisite.
- **`OXICLOUD_EXTERNAL_USERS_CAN_RESHARE=false`** env flag forbidding externals from being a grant's `granted_by`. Today an external user with `Permission::Share` can mint more external users via the email-grant flow. Soft policy; deferred.
- **Differentiated session lifetime for externals** (`OXICLOUD_EXTERNAL_REFRESH_TOKEN_EXPIRY_DAYS`). Today refresh-token TTL is uniform across all users. Deferred until operational data shows it matters.
- **`session_kind` discriminator on sessions emitted from magic-link.** Enables Option B-style scoped sessions (magic-link sessions only access granted resources, not the user's own home folder). Today every authenticated session is full-tier; magic-link only happens for users without home folders (externals), so the practical exposure is small. Deferred.
- **Admin-list-users surface that includes externals.** PR 6 makes `list_users` filter externals by default. The admin endpoint at `GET /api/admin/users` will eventually want an `include_external` query param so admins can manage externals (rename, deactivate, see grants). Not in scope of this work — the admin UI for externals is its own future PR.
- **Open Cloud Mesh (OCM) federation.** External users via OCM (federated partner servers) are a separate path; magic-link is one of several future external-identity providers. `ExternalIdentityLifecycleHook::on_user_created` design accommodates the `source` discriminator (`magic_link` / `oidc` / `ocm`).

## Recommended future event triggers (DON'T ship in this work)

Same convention as the lifecycle plan: a future event ships only when there's a concrete consumer.

| Future event | What would force it |
|---|---|
| `on_external_user_credential_set` | When an external user sets a password OR links OIDC — useful for an audit event ("alice@example.com is no longer magic-link-eligible") and for invalidating any outstanding magic-link tokens she has. Today the new tokens are simply unused; reaping them via this event would be cleaner. |
| `on_magic_link_resent` | If audit consumers want to see resend traffic distinguishable from invite traffic. Today the resend goes through the same code path as the initial issuance; an audit-distinguishable event isn't worth the trait surface yet. |
| `on_email_bounce` | When SMTP delivery fails permanently. Useful for surfacing "this user's email is dead" in admin UI. Requires a bounce-tracking infrastructure (SES-style webhook, custom bounce-mailbox monitoring) — out of scope. |

These are doc-only; their absence doesn't block anything.

## Two open questions I want to confirm via AskUserQuestion

None at this point — the conversation pinned every design decision. Proceeding straight to ExitPlanMode.
