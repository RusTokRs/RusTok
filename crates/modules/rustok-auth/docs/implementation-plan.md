# Implementation plan for `rustok-auth`

## Current state

`rustok-auth` is the mandatory core capability for JWT, claims, credential and
token configuration, lifecycle, OAuth, users, permissions (including
`AUTH_USER_PERMISSIONS`), and their public REST DTOs. `apps/server` supplies
persisted adapters, schema composition, and HTTP extraction only; it must not
regain auth business logic.

The admin surface is owned by `rustok-auth/admin` and follows the
core/transport/UI split. `AuthLifecyclePort`, `UserAdminMutationPort`, and
`OAuthAdminPort` are module-owned. Native adapters receive DB and runtime
extensions through `HostRuntimeContext`, while GraphQL and REST use the same
typed runtime contracts. The package consumes the host effective locale and
does not create a package-local locale fallback.

The shared client lifecycle is now owned by `rustok-auth/ui` (`rustok-auth-ui`):
`core`/`model` are framework-neutral, `transport` owns native and GraphQL/cookie
adapters, and `ui::leptos` binds one provider for admin and storefront. The server-heavy backend
is not a dependency of the client package. This is a source cutover, not a new
mounted storefront route or a claim of verified runtime parity.

**Client verification:** the locked Auth native/default and SSR tests, external public
contract and Auth admin SSR check passed on the cutover commit in [CI run 43575756113](https://github.com/RusTokRs/RusTok/actions/runs/43575756113).
Auth and Auth admin CSR WASM and the Product buyer island (including Auth hydrate) also
passed there. Full host builds, the new SSR rendering contract and other downstream
module tests remain to be completed. Exercise
real native/direct-cookie login, refresh, rejection and logout; tenant mismatch,
multiple providers, tabs and dropped requests; SSR snapshot/hydration; and mounted
admin/storefront/Notifications consumers. Source guards and standalone scope tests
do not prove cookie/network behavior. A same-document lease cannot serialize tabs
or cancel sent requests, and client scope is not authorization. The provider itself
has no external-invalidation listener; Cart's bridge invalidates its own state, but
other Auth projections can stay stale across roots/tabs. Route guards and some
storage-backed transports also bypass ready-scope getters. The standalone
storefront CSR entry now supplies the provider to both route branches, but its SSR
shell still has the pre-existing separate owner without an Auth provider or a validated snapshot.
Auth-dependent SSR children can therefore fail; implement request-validated, tenant-
and cache-safe SSR composition and prove it rather than treating an empty snapshot
as verified anonymity. The tracked historical Translation browser output fixture
now uses the real provider instead of constructing an
obsolete fake session; its stale generated standalone lockfile was removed and its old
screenshot is historical, not verification of this migration. Preserve the release
gates in the [Page Builder backlog](../../../../docs/modules/page-builder-backlog.md).

JWT follow-up source hardening is implemented: every authority read and CAS
is bound to the pinned authority identity; the caller MAC root is authenticated
even for an empty ledger; generation-zero/static HTTP admission is absent;
malformed replica-grant identifiers are rejected; observed lease expiry is
sticky; and production opens the external authority before application
migrations. Focused source regressions are authored but unrun; scoped checks
and real PostgreSQL contention/restore evidence remain pending.

RS256-only JWT target: [accepted ADR](../../../../DECISIONS/2026-11-25-rs256-only-jwt.md).
All repository-owned JWT signing and verification now use RS256 exclusively.
The mandatory generation-bearing file is the only key source; the document
contains no algorithm marker or alternate key source. JOSE verification pins
RS256 before exact kid lookup. The shared test fixture and POSIX dev/test
provisioner produce four independent RSA purpose pairs; no HS256 signer,
verifier, derivation, secret inventory, algorithm enum or default key
constructor remains. Durable fence and witness metadata retain a fixed RS256
assertion for authenticated-state continuity; any HS256 metadata is rejected.
Removing that marker requires a separately authorized offline conversion
preserving tombstones, revoked identities, generation and witness revision.
Scoped compilation, regression execution and independent-replica rollout
evidence remain pending. Do not reset non-disposable authority or witness
state; retained-state deployment remains blocked until the converter is
available.

JWT migration consolidation is blocked by the repository-wide
[admission guide](../../../../docs/guides/safe-database-migrations-and-squashing.md).
The candidate range is the three Auth-owned 2026-11-25 fence migrations:
`m20261125_000002_create_auth_jwt_generation_fence` creates the singleton,
`m20261125_000003_add_auth_jwt_cluster_lease` adds the nonnegative lease/drain
columns, and `m20261125_000004_add_auth_jwt_replica_grants` adds the grant
registry and untracked horizon, including the existing lease-to-untracked
backfill. A fresh baseline could create the final table and empty singleton
directly, but a previously exercised authority must retain its ledger,
leases, grants, identity tombstones and witness state. The witness revision
DDL and offline import are separately controlled activation/recovery
boundaries, not ordinary schema aliases. Keep all three migration identities
unchanged until release/module/container seals, original/candidate PostgreSQL
schema and data comparisons, installer/independent-store paths, rollback
guards, retained-state disposition and trusted replacement admission are
verified. The repository bootstrap record reports generation zero; empty
GitHub Releases alone do not establish the complete stable boundary.
The SQLite fixture now uses the same owner migration inventory as independent
authority import. Fresh up/down/up and exercised-state rollback-refusal
regressions are authored, unrun; they do not certify consolidation.

JWT rollback witness: source-implemented exact authenticated checkpoint,
monotonic mutation revision, witness-first commit, offline recovery, verified
TLS and sticky local mismatch fencing. Production requires a third independent
store. Scoped compilation, regression execution, real PostgreSQL commit-fault,
restore/failover and TLS evidence remain pending.

Independent JWT authority: production requires a pinned external PostgreSQL store;
Auth CLI imports the offline ledger once and revokes against that same store.
Application DB rollback cannot erase its key history. Independent authority
restore and session/OAuth rollback protection remain separate. Scoped compilation,
CLI and real PostgreSQL cutover/restore evidence remain pending.

JWT clock discontinuity detection: dual local deadlines, sticky runtime and
shared authority fault latches, bounded DB time samples and conservative key
retirement/promotion margins are source-implemented. No schema change.
Scoped compilation, regressions, promtool and independent PostgreSQL/VM
clock fault-injection and suspend tests remain pending.

JWT operational observability: authenticated cluster gauges, local lease state,
bounded operation counters, ACK-attempt timing, pending generation age and
initial alerts are source-implemented. Compilation, tests, promtool and
independent-replica scrape/failure evidence remain pending.
See [owner contract](README.md#operational-observability).

## FFA/FBA boundary

- FFA status: `phase_b_ready`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA registry and static/runtime evidence:
  `crates/modules/rustok-auth/contracts/auth-fba-registry.json`,
  `crates/modules/rustok-auth/contracts/evidence/auth-capability-static-matrix.json`,
  and `crates/modules/rustok-auth/contracts/evidence/auth-runtime-fallback-smoke.json`.

## Open results

1. **Completed — remove implicit OAuth grant expansion for manifest-created applications.**
   `oauth_apps::Model::supports_grant_type` now enforces exact persisted grant
   membership. Manifest-managed first-party admin/storefront producers explicitly
   declare `refresh_token` alongside `authorization_code` and
   `client_credentials`, preserving their intended refresh rotation behavior.
   Regression coverage rejects an undeclared refresh grant for both manual and
   auto-created applications and accepts the explicit auto-created grant set.

2. **Capture runtime parity evidence for user and OAuth mutations.** Exercise
   the browser/admin path and the owner-owned GraphQL/native paths for the same
   successful and rejected operations.
   **Depends on:** an environment with persisted lifecycle/OAuth adapters and
   test identities.
   **Done when:** reproducible evidence covers tenant scope, RBAC, canonical
   error mapping, and host-resolved locale propagation; only then consider a
   `parity_verified` promotion.

3. **Preserve boundary parity as auth flows evolve.** Add or change token,
   credential, OAuth, or user-management behavior only through the typed module
   ports and published REST/GraphQL contracts.
   **Depends on:** the change-owning public contract.
   **Done when:** the module README, metadata, and FFA/FBA evidence describe the
   same runtime surface without a server-local bypass.

4. **Completed — provide bounded identity reads for owner-owned operations.**
   `AuthUserBackfillReadPort` exposes only tenant-scoped user id, email and
   display-name data in deterministic creation order for profile provisioning.
   `AuthUserBackfillDbReader` now enforces a strict `1..=500` batch bound,
   performs checked SQL-limit conversion and redacts storage/row-read failures.
   The host-independent reader continues to use only an explicit database
   handle, while the Profiles CLI delegates through the owner contract without
   importing auth persistence models.

5. **Keep OAuth bootstrap in the auth-owned CLI adapter.**
   `rustok-cli oauth create-app` creates the development application through
   `rustok-auth/cli`, an explicit database handle, and a required explicit
   `--tenant-id`. It must never infer the first active tenant for a credential
   write. Base application identity and localized display copy are persisted in
   one transaction against the current translation-table schema. The server does
   not register a task for this operation.

6. **Keep session maintenance in the auth-owned CLI adapter.**
   `rustok-cli auth sessions-cleanup` removes expired auth sessions without the
   server task bridge.

7. **Completed — keep bootstrap identity provisioning in the auth owner.**
   `AuthUserBootstrapDbWriter` provides idempotent tenant-scoped user creation
   from an explicit database handle for installer and future standalone seed
   composition. Email lookup is canonicalized case-insensitively, database/row
   failures are redacted, and unsupported backends fail closed without process
   panics. RBAC role assignment remains a separate owner boundary and installer
   seed composition keeps identity and role assignment in one caller-owned
   transaction.

8. **Provide a lightweight owner-owned OAuth transaction regression target.**
   The current server-lib test target compiles embedded UI, storage, cloud, and
   unrelated domain composition before it can exercise OAuth code/refresh CAS.
   Move the persistence algorithm behind an auth-owned or narrowly shared adapter
   contract whose SQLite concurrency tests do not require the full server graph.
   **Done when:** authorization-code and refresh-token exactly-once tests run
   without compiling `apps/admin`, storefront hosts, cloud SDKs, or unrelated
   domain modules, while the server provider consumes the same tested boundary.

9. **Completed in code, verification pending — 2026-10 auth audit remediation
   (P1→P2→P3).** Canonical email/display-name validation and a length-based
   password policy now run on every credential path (register, invite accept,
   reset, change-password, admin create/update, superadmin bootstrap), with
   unique-violation races mapped to `EmailAlreadyExists`. Delegated and
   service principals can no longer authorize or consent as the user
   (`AuthAdminMutationContext.principal_kind`, fail-closed; REST authorize /
   consent / browser-session endpoints require a direct user). Login gained
   dummy-Argon2 timing equalization, a persistent per-account lockout
   (`users.failed_login_count` / `locked_until`, migration
   `m20261025_000001`), and fail-closed rate-limit defaults. OAuth consent
   can be re-granted after revoke (partial unique index, migration
   `m20261025_000002`) and now replaces scopes instead of merging them.
   Refresh rotation records the previous token hash and revokes the lineage
   on replay after a 30-second grace window, with a 90-day absolute session
   cap (migration `m20261025_000003`). OAuth app writes are column-scoped so
   they cannot revert secret rotation; GraphQL internal errors no longer leak
   database text; reset/verification responses never carry tokens. Email
   verification is enforced at login and refresh when
   `features.email_verification` is enabled, and registration sends the
   verification email. Invitations are a complete lifecycle
   (`issueUserInvite` / `revokeUserInvite` / `userInvites`, issuance ledger
   `auth_invites`, migration `m20261025_000004`) gated by
   `features.invites_enabled` and `features.invite_only_registration`.
   Redirect URIs are validated strictly (https or loopback http, no fragment
   or user info, custom schemes only for mobile apps); credential fields are
   excluded from entity serialization; `UserAccountRegistered` is published
   transactionally; the superadmin bootstrap no longer re-promotes an
   existing account on restart.
   **Verification status:** written without a local Rust toolchain; not yet
   compiled or tested. Run the Verification list below plus targeted server
   suites (`auth_lifecycle`, `oauth_app`, `oauth_token_service`,
   `auth_admin_mutation_provider`) and `cargo fmt` before merge.
   **Formerly deferred architectural items, now implemented (same blind
   verification status):** browser sessions no longer persist credentials —
   new `POST /api/auth/session/{login,register,refresh,logout}` endpoints
   keep the refresh token in an HttpOnly cookie for browser-direct mode,
   SSR/BFF hosts hold it in their own HttpOnly cookie via native server
   functions, OAuth access tokens are registered in `oauth_tokens` and revoked by family, and the admin's
   LocalStorage-to-cookie JS bootstrap is replaced by a server-to-server
   verified snapshot exchange; RBAC permission checks are snapshot-first
   with authoritative fall-through (OAuth scope narrowing honored);
   Argon2 hashing and credential validation run before user-creation
   transactions via `prepare_new_user`; OAuth refresh tokens have lineage
   families with a 90-day absolute cap and post-grace reuse detection that
   revokes the whole family (migration `m20261025_000005`); concurrent
   first-time consent grants retry through the update path instead of
   returning a unique-violation 500; invites support tenant-defined custom
   role slugs with the authority ceiling enforced at issuance; invite
   management and acceptance UI screens are built (Users page panel +
   `/accept-invite`); `user.session.*`, `user.password.*`,
   `user.email.verified`, and `user.invite.*` audit events publish inside
   the mutations' transactions. The admin-mutation port error taxonomy was
   assessed and is already typed (`AuthAdminMutationError` variants matched
   exhaustively at the GraphQL boundary); `AuthSettingsOverrides::apply` is
   live runtime wiring, not dead code.

## Purpose-separated host MACs (source only)

Auth owns HKDF-SHA256 derivation and HMAC-SHA256 for password-reset credential
binding and OAuth browser confirmation. Canonical proof labels and wire
encodings are unversioned, with strict lowercase full-length tags and canonical
OAuth expiry encoding. Previous forms and reset links intentionally fail.
Access JWT signatures, durable fence MAC domains and refresh/session storage
are unchanged. Stop old host writers before deploying new instances. Source
regressions authored, unrun. See [canonical Auth MAC proofs](../../../../DECISIONS/2026-11-25-canonical-auth-mac-proofs.md).

JWT purpose keyrings, mandatory key IDs and verification retirement are
source-implemented. All repository-owned JWT signing and verification are
RS256-only; the mandatory complete keyring file supplies four independent
purpose pairs and server bootstrap performs durable admission before
application migrations and listener startup. The keyring document has no
algorithm selector; JOSE verification pins RS256 before exact kid lookup.
No HS256 signer, verifier, derivation or fallback remains. Durable metadata
retains a fixed RS256 assertion for continuity; persisted HS256 fails closed,
and incompatible retained stores remain blocked pending the authenticated
offline converter. Scoped compilation, regression execution and
independent-replica rollout evidence remain pending. Reissue obsolete
development tokens and shared-key special links.
Canonical RSA modulus identity, private/public pair enforcement at signing,
strict JOSE header admission, post-signature retirement fencing and checked
signed-time conversion are source-implemented. Focused regressions are
authored but not executed.
Immutable prepared JWT snapshots are source-implemented: all-purpose admission,
shared once-only initialization, private key configuration and candidate detachment.
Process-local hot reload now shares a publication fence across all AuthConfig clones;
the optional complete file document is checked before startup and polled by the host.
Auth-owned durable generation admission now persists the current metadata and
lifetime identity bindings before startup or local publication; source regressions
cover rollback, conflicts, terminal removal, staging and restart of the store.
A database-serialized cluster lease/drain barrier now fences stale HTTP hosts
at the JWT crypto boundary, including static-to-file enrollment. Rolling
replica grants now acknowledge actual verifier metadata, permit compatible
staging/promotion without a global drain, and wait for incompatible older
grants to expire or be acknowledged after local application. Emergency revocation now records permanent enrolled identity bans in the authenticated ledger and fences observed local generations via lease renewal; real multi-replica
runtime evidence is pending.
Scoped checks are pending. See the
[owner contract](README.md#jwt-keyring-and-rotation).

## JWT and CSRF hardening (source only)

Browser confirmation MAC binds the direct session and bounds expiry;
previous forms require restart. RS256 host config now also validates the
symmetric secret. JWT time claims and chrono duration construction fail closed.
Source regressions authored, unrun. Deployment must provision the symmetric
secret even with RSA signing; keep server clocks synchronized.

## OAuth revocation client fence (source only)

RFC 7009 revocation revalidates the active client identity, type and secret hash
under the app lock before token lookup. Stale authentication returns invalid_client;
unknown tokens still succeed for authenticated clients. Source regression authored,
compilation and runtime verification pending.

## OAuth authorization session fence (source only)

Authorization issuance rechecks the direct session, its subject, credential
expiry and email policy under app -> user -> session locking. Browser approval
and code persistence now commit together; rejected or failed code issuance
cannot leave a new grant behind. Source regressions authored, unrun.

## OAuth consent and grant fences (source only)

Consent re-grant invalidates pending codes in its transaction; code exchange
rejects a newer consent epoch and checks the refresh grant under the app lock
before persisting prepared credentials. OAuth replay grace no longer accepts
future timestamps or truncates elapsed time to whole seconds. Focused source
regressions authored; compilation and runtime verification remain pending.

## Direct session mutation ordering (source only)

Refresh and bulk session revocation use the shared tenant-scoped user mutation
fence before acquiring session locks, including history-only recovery and
replay. Bulk self-service revocation checks the current session again under
lock. SQLite reserves its writer at the user fence before reading the evidence. Regressions exercise lock ordering and stale current
session rejection; PostgreSQL contention, compilation and the remediation gate
remain maintainer-run.

## Direct session refresh history (source only)

The platform migration preserves the available previous digest and timestamp; every
subsequent rotation records immutable history in the same transaction. Replays of any
recorded generation outside grace revoke only the tenant-scoped session. The current
session and previous-cookie logout contract are preserved. Older overwritten digests
cannot be recovered. Apply the migration before deploying the new host, with old host
writers stopped. Regression definitions, PostgreSQL backfill, cascade, rollback and
concurrency checks remain unrun.

## Proof-bound direct session refresh recovery (source only)

The host accepts the owner-defined secret retry header for raw REST and
GraphQL HTTP refresh, and derives the cookie REST proof from its envelope; derives the opaque successor under the
independent MAC root, verifies the current digest and history under the
user/session fences, and preserves the original refresh deadline and history
on recovery. The GraphQL provider preserves rotation conflicts as CONFLICT.
See [the recovery ADR](../../../../DECISIONS/2026-11-26-session-refresh-retry-proof.md).

The cookie-session REST owner now issues one canonical HttpOnly token/seed
envelope and derives the proof from the seed, resolved tenant UUID and
presented token. Next Admin relays the owner envelope unchanged; its former
local derivation has been removed. One bounded automatic retry handles
ambiguous upstream refresh results; the domain GraphQL operation is never
automatically replayed. The BFF rejects duplicate/malformed cookies and
redirects, checks the refreshed subject and seed continuity, and does not
clear cookies on transient refresh failures. Logout requires explicit ok.
Existing sessions require coordinated revocation and new sign-in.

Leptos shared native SSR/hydrate and Admin SSR middleware now source-wire
a canonical HttpOnly envelope, independently generated OS CSPRNG seed,
generation-bound proof, redirect refusal and one bounded ambiguous retry.
Explicit auth functions own their cookie transitions; middleware does not
perform a competing rotation or overwrite an explicit handler Set-Cookie.
The shared provider attempts refresh instead of revocation on access expiry.
Old native bare cookies require coordinated session revocation and new sign-in.
Scoped proof, cookie admission, status and mock HTTP retry regressions are
authored and unrun. Only native/SSR cryptographic dependencies are enabled.

Direct Leptos CSR now uses the Auth-owned HttpOnly envelope with no
JavaScript-readable secret storage. A shared exclusive Web Lock bounds
cookie mutation admission and serializes cooperating tabs through response
parsing and one ambiguous refresh retry. The secure-context browser
transport refuses redirects, uses no-store and same-origin Fetch, and
requires Web Locks/AbortSignal.timeout. Cookie admission, proof derivation,
retry and storage regressions are source-authored; mounted browser and
failure-injection verification remain unrun. Out-of-order noncooperating
responses, stale intents and cross-tab provider projections remain open.

Remaining implementation: standalone Storefront SSR validated composition,
Next Storefront (currently observation-only, without its own refresh flow),
mobile/native consumers, generated API header reference updates, and
header secrecy in proxies/telemetry. Next Admin browser/runtime and
cross-replica evidence, late cookie delivery and Auth.js logout failure
handling remain pending. OAuth refresh-family recovery is separate.
Run scoped Auth and server compilation, retry-proof and lifecycle tests,
mounted REST/GraphQL/CORS contracts, PostgreSQL contention and cross-replica
lost-response injection, plus the remediation gate. None has been executed.

## Password recovery parity (source only)

REST and GraphQL/native recovery requests delegate to the same host lifecycle
service. Account-dependent preparation failures preserve the generic response;
inactive accounts do not receive unusable reset links. Confirmation now includes
the exact observed mailbox in its atomic credential CAS, preventing a concurrent
mailbox change from authorizing a stale reset. Run `cargo test -p rustok-server
--lib auth_password_reset` plus mounted REST and GraphQL recovery checks.
Compilation, regressions and remediation gate remain maintainer-run.

## Shared Auth throttling budget (source only)

REST and GraphQL use the same per-IP key in the shared Auth limiter, including
one unknown-IP bucket. Trusted bearer claims cannot partition this strict budget;
GraphQL password changes and logout now consume it as well. General API and OAuth
key dimensions remain unchanged. Source key and shared-counter regressions are
unrun; mounted middleware/GraphQL and Redis verification remain pending.

## GraphQL credential throttling remediation (source only)

The credential limiter now accounts for separate aliased resolver calls, ignores
unselected named operations, and traverses cyclic/shared fragment graphs safely
before validation. Focused classifier regressions are authored but unrun. Run
`cargo test -p rustok-auth --features graphql --lib graphql::rate_limit` and the
host GraphQL integration checks before release. Compilation and the remediation
gate are pending maintainer verification.

## Verification

- `npm run verify:auth:ui-boundary` and `npm run test:verify:auth:ui-boundary`
- `cargo test --locked -p rustok-auth-ui --lib` and `--features ssr`
- `npm run verify:auth:admin-boundary`
  (`scripts/verify/verify-auth-admin-boundary.mjs`)
- `npm run verify:ai:fba-baseline`
- `cargo xtask module validate auth`
- `cargo xtask module test auth`
- `cargo test -p rustok-auth-cli oauth_create_app -- --nocapture`
- `cargo test -p rustok-auth-cli development_app_uses_current_translation_schema`
- `cargo check -p rustok-auth-admin`
- Targeted auth/RBAC server tests when runtime wiring changes.

## Change rules

1. Keep auth lifecycle, OAuth, user mutation, and permission policy in the
   owning module.
2. Update the root README, local docs, and `rustok-module.toml` with a public
   or metadata change.
3. Update this status block and `docs/modules/registry.md` in the same change
   when the UI or transport boundary changes.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `blocked`
- Last verified at (UTC): `2026-07-30`
- Scope inspected: `auth ownership; JWT and credential lifecycle; OAuth app, authorization-code and refresh-token paths; tenant-qualified admin mutations; translation-table migration and runtime localization; auth-owned CLI bootstrap adapter; explicit operational tenant selection; implicit grant expansion`
- Findings: `P0=0, P1=7, P2=0, P3=2`
- Fixed in this pass: `preserved the four previously repaired P1 defects; fixed the post-migration rustok-auth-cli OAuth bootstrap failure by deleting raw writes to removed oauth_apps.name/description columns and persisting the base row plus en translation in one transaction; added a SQLite regression against the current translation-table schema; removed first-active-tenant inference from oauth create-app and required an explicit tenant UUID before credential persistence, with missing, invalid and valid UUID regressions; closed implicit refresh_token authority by enforcing exact persisted grant membership and explicitly declaring refresh_token in first-party manifest producers`
- Remaining risks or blockers: `same-SHA auth CLI execution is pending in GitHub Actions; PostgreSQL forward/down migration smoke and browser/runtime mutation parity evidence remain required; lightweight owner-owned OAuth code/refresh regression target remains P3 verification debt`
- Evidence: `source inspection confirms tenant-qualified OAuth client, consent and admin lookups; authorization-code and refresh-token replacements use transactional compare-and-set; oauth_app_translations uses VARCHAR(32), tenant-composite FK and unique tenant/app/locale identity; auth CLI no longer imports rustok-tenant or rustok-api, requires --tenant-id before OAuth credential writes and contains explicit missing/invalid/valid UUID unit cases; tenant FBA guard forbids read_default_active_tenant in the credential path; connector-only local execution remains unavailable because github.com DNS resolution fails and is classified as an environment limitation`
- Next action: `run the targeted rustok-auth-cli regressions and canonical auth module checks; rerun PostgreSQL migration smoke; then revisit this blocked item during closing gates`
- Resume command: `cargo test -p rustok-auth-cli oauth_create_app -- --nocapture && cargo test -p rustok-auth-cli development_app_uses_current_translation_schema && cargo xtask module validate auth && cargo xtask module test auth`


## P8 — OAuth access-token revocation fence

OAuth access JWTs carry a `jti` bound to a durable `oauth_tokens` access row.
The host extractor fails closed unless that row is active and matches the
presented hash, client, tenant, grant, and subject. RFC 7009 and security
revocation paths revoke access and refresh rows together. Delegated rows store
the consent epoch so revoke/re-grant cannot resurrect older credentials.
