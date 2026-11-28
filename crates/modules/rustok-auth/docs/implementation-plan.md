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
do not prove cookie/network behavior. The shared provider now coordinates cooperating browser operations and invalidates
foreign projections; this does not authorize requests or cancel sent operations.
Both Storefront SSR render branches mount the Auth provider and inherit a verified
request-scoped snapshot when one is available. An empty snapshot is not proof of
absent credentials. Standalone runtime/tenant-policy composition and mounted
verification remain open. The tracked historical Translation browser output fixture
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
   `rustok-cli auth sessions-cleanup` calls the Auth owner's session-only
   retention operation without a server task bridge, preserves the same
   30-day horizon as the worker and refuses dry-run.

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
unknown tokens still succeed for authenticated clients. Authorization-code
and refresh-token replay paths revalidate that same active client snapshot and
the requested grant under the mutation lock before family revocation. Source regressions authored,
compilation and runtime verification pending.

## OAuth authorization session fence (source only)

Authorization issuance rechecks the direct session, its subject, credential
expiry and email policy under app -> user -> session locking. Browser approval
and code persistence now commit together; rejected or failed code issuance
cannot leave a new grant behind. Source regressions authored, unrun.

## OAuth consent and grant fences (source only)

Consent re-grant durably revokes subject-bound access and refresh rows and
invalidates pending codes in its transaction, independently of epoch timestamp
resolution. Manual and first-party manifest security-policy updates atomically
revoke app credentials, pending codes and consents. Equivalent policy sets
and presentation-only edits preserve credentials. Code exchange
rejects a newer consent epoch and checks the refresh grant under the app lock
before persisting prepared credentials. OAuth replay grace no longer accepts
future timestamps or truncates elapsed time to whole seconds. Focused source
regressions authored; compilation and runtime verification remain pending.

OAuth issuance additionally rechecks the locked authorization session after
consent writes and after code insertion; future-dated and exactly expired codes
cannot be consumed, and prepared token expiry is rechecked before committing
a code exchange. Service access responses report remaining durable lifetime.
The Auth signer now returns exact signed expiry metadata and accepts the
absolute family cap before signing. Service and delegated access registrations
use that exact deadline; response lifetime is calculated after usage telemetry.
Future-dated prepared credentials fail closed. Old registrations retain their
stored expiry and the extractor continues taking the earlier deadline.
Boundary regressions are authored; compilation and transaction tests remain open.
Live-runtime OAuth exchange and access extraction now use the shared Auth
credential clock at admission, locked mutation and response boundaries. Strict
serialized wall observations latch backward steps, including subsecond steps;
refresh replay classification fails without mutating on an unavailable clock.
Browser confirmation verification now returns the authenticated expiry and
the code-issuance transaction rechecks it with the shared credential clock
after locks, before consent, before code insert and before commit. Code issuance
also verifies its persisted expiry before response; a post-commit fault
suppresses publication rather than undoing a durable grant. The shared
application clock now corroborates DB time at OAuth admission, time-dependent
replay classification, locked consent/code/token mutations and pre-commit,
and final common access extraction. Standalone consent grants use the verified
host timestamp; future-dated access registrations fail closed. Retention and
credential paths share one process-local application-clock latch, independent
of JWT signing authority. Retention retains the fixed conservative cutoff;
the post-commit response checks the local latch without another DB round trip.
Direct login/registration and refresh/recovery now sample the application
DB and live credential clocks under the existing mutation fences and before
commit; login failure counters use corroborated time. Direct access JWT expiry
is bounded by the durable session and returned exactly from signing; both
response lifetimes are recomputed after commit. Future rotation timestamps
cannot classify replay as theft. Other recovery mutations, durable cross-restart
and cross-replica rollback remain open. Independent processes require reliable
time and a single trusted primary. Source regressions unrun.

## OAuth family lifetime and replay evidence (source only)

New access registrations and refresh rows cannot exceed the original family
deadline; prepared credentials are checked under the app/subject fence before
consumption and delegated responses report the remaining access lifetime.
Grace requires matching rotation timestamps and valid temporal evidence, not
merely a recent revocation. Focused boundary and transactional regressions
are unrun. Existing pre-change access registrations retain their stored expiry;
canonical family identity is now source-wired: non-optional Rust UUID,
PostgreSQL NOT NULL/deferred root FK, SQLite write/deletion guards, one
unrevoked refresh per family and immutable ownership/epoch identity.
The migration backfills only old NULL-as-self roots and rejects inconsistent
or forked history. Retention waits for all family members and preserves
authorization codes until no linked token remains. Migration/rollback and
PostgreSQL contention tests remain unrun. Proof-bound OAuth refresh
recovery is source-wired using an optional secret singleton header and an
independent purpose over tenant/app/family/predecessor/proof. The owner
rechecks current app, subject, consent, exact scopes, live successor and
exclusive grace under lock; recovery inserts only a fresh access row.
The canonical CORS allowlist already admits the proof header; HTTP edge
redaction now explicitly covers it for direct and OAuth refresh. Source
regressions cover a superseded successor and delayed admission beyond grace.
Client integration and cross-replica fault-injection evidence remain open.
Root resolution now rejects unconsumed roots, mismatched consent epochs,
widened descendant scopes and future timestamps. The app/subject-locked
rotation refuses a family with multiple live refresh branches. Focused
regressions are authored but unrun.

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
GraphQL HTTP refresh, derives the cookie REST proof from its envelope, and
derives the opaque successor under the independent MAC root, verifies the current digest and history under the
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
failure-injection verification remain unrun. The shared Leptos provider now
extends the lock through publication for CSR and hydrate, rechecks the
captured epoch at admission, clears foreign projections on existing
notifications, and tags its atomic verified memory pair with the completed
epoch. Native authenticated calls refuse stale/absent credentials and recheck
before dispatch. Refresh rejects tenant/subject/lineage replacement. Source
regressions cover changed, missing or unavailable epochs and identity
substitution. Routine refresh conservatively invalidates foreign projections.
Expected-lineage admission is source-wired for raw/cookie REST refresh/logout
and GraphQL HTTP refresh. The strict optional non-nil canonical UUID header is
checked before rotation/recovery/reuse revocation. Expected logout proves the
digest belongs to the locked tenant lineage (including retained history);
missing/mismatched credentials do not clear cookies. Leptos projects the
expected ID through CSR/hydrate and Next Admin pins Auth.js to the direct
session, compares tenant/subject/lineage before dispatch, and avoids clearing
cookies on rejected refresh. Next Admin explicit logout is now source-wired:
the closed credentials-only Auth.js route admits same-origin POST with a
frozen expected lineage, revokes through Auth before any cookie cleanup,
then invokes the private Auth.js sign-out helper and clears Rustok cookies.
UI header/sidebar and credential-change flows share the transport; failures
are localized and do not repeat a committed credential change. Failed login
does not clear unrelated cookies. Generic Auth.js signout bypass is removed.
Regressions cover current/historical mismatch, matching proof recovery,
historical logout retry, header parsing, adapter forwarding, route admission,
cleanup ordering and explicit response confirmation.
Out-of-order noncooperating responses, SSR navigation, discovery without
expected identity, expired Auth.js identity recovery, post-revocation cleanup
failure and mounted verification remain open.

Storefront SSR document routes now use the shared Auth SSR adapter and an
already-resolved tenant slug/UUID, mount the provider in both render branches,
and prevent credential-bearing documents from conditional revalidation.
Admin and Storefront share cookie admission/response handling; assets, RSS,
health, HEAD and speculative document fetches do not rotate. Native Admin
cookie-consuming calls and Page Builder preview/intents retain their explicit
paths. Source regressions cover tenant mismatch, navigation admission,
cookie precedence and scoped SSR render identity. A snapshot is not a
validated backend principal and does not supply missing standalone runtime
services.

Remaining implementation: standalone Storefront runtime/tenant-policy composition,
Next Storefront (currently observation-only, without its own refresh flow),
mobile/native consumers, generated API header reference updates, and
header secrecy in proxies/telemetry. Next Admin browser/runtime and
cross-replica evidence, late cookie delivery and post-revocation cookie
cleanup recovery remain pending. OAuth refresh-family recovery is separate.
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

## Recovery lifecycle continuation (2026-11-26)

- Recovery revision follow-up: complete RBAC user ActiveModel fixtures now leave the database-owned recovery revision unset, so inserts use the migration default. Source-only inspection; compilation and execution remain unverified.

- Invite revocation revalidates active actor and current module-filtered `users:manage` under actor/invite/module/ordered actor-role fences, including idempotent retries. Issuer and target-role validity are not prerequisites. Shared RBAC role-row query retains the invitation binding format. Source regression covers stale authority, event failure rollback, successful retry, idempotence and accepted receipt conflict; unrun. PostgreSQL contention and artifact/runtime grant fences remain open.

- Admin custom-role provisioning uses the shared direct creation path after checking active actor and both request/persisted delegation under module and ordered role locks. No temporary Customer. Source regression covers stale request grants and exact final membership/revision; unrun. Built-in admin creation and cross-owner contention remain open.

- Custom-role invite acceptance assigns the locked persisted target directly, sharing validated account insertion with built-in creation. No intermediate Customer membership or role bootstrap; the receipt, user and final role remain atomic. Source regression checks exact membership, single revision increment, absent Customer and rollback on missing role; unrun. Admin provisioning and full PostgreSQL contention are separate open paths.

- Invite acceptance locates the issuer outside its transaction, locks issuer before invite (matching issuer deletion/FK SET NULL order), and revalidates current ledger digest, issuer epoch and revocation. A committed receipt short-circuits before module/RBAC locks; the unique digest insert remains authoritative. Stale-locator and revocation source regressions authored, unrun; PostgreSQL lock contention and remaining cross-owner lock order unverified.

- Tracked recovery email dispatch independently admits application time, revalidates signed reset/verification JWTs against the live keyring, and carries the committed opaque email-change deadline. Clock/expiry/credential denial skips the sender with a fixed diagnostic and skipped metric; old-mailbox notifications are unaffected. Source deadline, dispatch and stored-expiry regressions authored, unrun. Dispatch now also re-reads current account identity and reset MAC, skips already-verified accounts, and checks the exact pending email-change challenge and owner in one statement. Source regressions cover recipient mismatch, password/mailbox ABA, verification completion, replacement and tenant isolation. Durable admission now runs after account/challenge reads; JWTs are revalidated after that final wait. The missing-clock fixture retains a valid account/token, and opaque dispatch tests both deadline sources and deletion. SMTP latency, changes after the read and durable delivery remain separate open boundaries.
- Reset, verification and invite JWT issuance now share the access-token post-signing live-clock and signed NumericDate deadline check. A deterministic regression covers fractional-second expiry, equality, backwards observation and epoch-zero issuance. Source regression unrun; application-domain publication and commit-time guarantees remain separate.
- Email-change issuance and confirmation bind the existing user recovery revision; the Auth migration invalidates pre-cutover pending challenges. Mailbox ABA and stale issuance source regressions are authored.
- Reset, verification, email change and invite acceptance recheck credential deadlines after transactional writes, immediately before requesting commit; authenticated email change also rechecks the locked proving session. Invite acceptance checks signed and ledger expiry.
- PostgreSQL/SQLite fresh and incremental migration, rollback, contention and transport execution evidence remain pending. Source regressions are unrun. No transport or readiness status promotion.
- Durable high-water time and commit-time deadline semantics remain separate unresolved boundaries.
- Static module policy binding combines owner lifecycle cursor sequence, active composition revision and exact policy digest under transaction locks. Both invitation paths resolve and filter persisted grants without TTL cache; the host passes its registry explicitly during provider composition. Conservative tenant-wide invalidation; artifact/runtime policy, direct bypass writes, full PostgreSQL contention and execution evidence remain open.
- Static role-grant epochs bind issuer and destination roles, including role identity, under ordered role-row locks. Built-in targets are materialized before signing; bootstrap conflict winner is not reconciled by the loser. Legacy invitations without a role binding are revoked. Source regressions for grant ABA, rollback, no-ops, unrelated roles and permission identity are unrun. Artifact grants are separate from static invite authority; module policy and PostgreSQL contention evidence remain open.
- RBAC membership revision advances atomically on relation mutations. Invite issuance/acceptance bind it under the issuer fence; missing bindings fail closed. Source regressions cover rollback, ABA, no-op, owner transfer, cascade and overflow; compile/migration/PostgreSQL contention remain unverified.
- Invite issuer account epoch is bound in the issuance ledger; acceptance compares it under the issuer lock. Auth migration revokes unconsumed legacy invites, preserves receipts, and revokes bound pending invites on schema down. Source regressions cover legacy binding, status ABA, unrelated profile writes, tenant mismatch and migration cutover; execution remains pending. RBAC membership/grant/module-policy history remains separate.
- Invitation issuance now locks the issuer, requires active account status, and checks current persisted permission and role authority independently of the request ceiling before signing. Acceptance and issuance share the delegation validator. Missing/inactive/ungranted issuer regression authored, unrun. Concurrent issuer-role permission edits, module-policy history and irreversible ABA rejection still require an RBAC-owned authority contract; do not use global cache invalidation generation as per-issuer authority.
- Targeted reclamation now releases expired or account-invalid foreign mailbox reservations before issuance under a separate previous-owner mutation fence. The exact challenge is re-read, live reservations are preserved, and broad cleanup remains with retention. Concurrent replacements after preflight may still return a unique-index conflict. SQLite source regression covers ABA, live reservations, failed replacement rollback, tenant isolation and old-token rejection; PostgreSQL contention and execution remain unverified.

- Verification records the corroborated post-lock admission timestamp, not a separate wall-clock observation; repeated verification preserves that timestamp. Email-change confirmation resolves competing mailbox claims at the unique-index write inside a savepoint; the conflict consumes the exact challenge without changing the user, with a fresh deadline/session check after the wait. SQLite regression source exercises the conflict write and replay rejection; PostgreSQL savepoint/contention evidence and compilation remain unrun.

## Durable application clock admission (2026-11-26)

- Auth migration adds singleton host/DB millisecond floors; admission commits independently of domain rollback, serializing before observation. Missing state fails closed.
- Initial wiring covers direct registration/login/refresh/session creation, access extraction (including post-policy admission), OAuth token exchange/code issuance/strict consent, password reset, verification, email change, invite issuance/acceptance, and retention.
- Existing in-transaction corroboration checks the durable floor without taking the global writer lock. Subsequent observations are not independently durable; full expiry non-resurrection, WebSocket and remaining consumers, and independent JWT authority continuity remain open.
- Source regression authored for rollback/recreated guard/missing state; PostgreSQL independent-pool contention, synchronized replica skew, migration and runtime checks remain unrun. No readiness promotion.

## Witnessed authority durable expiry fence (2026-11-26)

- Time-sensitive independent authority reads durably advance the authenticated witness DB clock floor before returning; mutations recheck under both locks and preserve it. Zero-row checked CAS operations persist their observed floor too. Clock-only updates keep the ledger revision and use the uncertain-commit guard. Emergency revocation and offline recovery preserve the floor without relying on expiry decisions.
- PostgreSQL application admission forces synchronous commit. No new DDL for the witness format extension; offline old-writer/lease drain and trusted-clock legacy enrollment required.
- Source regression covers restarted authority refusing a lower DB clock while emergency revocation retains the floor. Compilation, PostgreSQL multi-pool/failover/cancellation and WAL/latency evidence remain unrun.
- Application late observations, request-path JWT host timestamps, unwitnessed development mode, witness/application-store rollback and cross-replica contention optimization remain open. No production readiness promotion.

## Independently committed application late observations (2026-11-26)

- Server host and session-cleanup CLI configure a separate clock writer pool. PostgreSQL samples the source and same primary writer under the singleton lock and commits the floor before returning the decision sample; a domain rollback cannot erase it. Uncertain clock commits trip the process latch.
- PostgreSQL pins the startup routing identity and checks the source floor before the first clock sample; an observed source rollback fences before the independent write. SQLite can use an offline-bound persistent sidecar with FULL/EXTRA sync, persistent journal or WAL, exact copied floor and matching UUID. MEMORY/OFF journals fail closed on the actual writer connection. No same-file writer, automatic reseed, or bound-file fallback. Unbound SQLite and unmanaged embedders retain admission-only protection.
- Late session, password/email recovery, access-registration, OAuth-family and retention observations use the durable path when configured. Access extraction checks deadline crossing during commit; exact domain commit/publication instant, in-memory JWT and WebSocket timers remain separate.
- Source regressions authored for independent rollback survival, restart, binding, unsafe sidecar journaling, source-floor rollback, zero-row witness CAS and uncertain commit. Compilation, real-file SQLite, PostgreSQL multi-pool/failover, cancellation injection and throughput evidence remain unrun. No production readiness promotion. See the clock ADR for deployment and recovery constraints.

## Final credential publication and WebSocket fencing (2026-11-27)

- Direct session service and REST/cookie/lifecycle adapters revalidate exact JWT eligibility and original deadlines, including proof-bound recovery grace. OAuth checks signed access eligibility and all prepared token deadlines at response. No compensating revocation of an already committed rotation.
- Common access extraction checks exact JWT eligibility after asynchronous policy IO; WebSocket caps lease by wall and monotonic deadlines, latches expiry, and checks local authority before connection publication and send.
- Source regression for monotonic expiry; tests, compilation, live rotation and blocked socket writes remain unverified. Post-commit observations are not all durably persisted; no readiness promotion.

## Cross-replica authority admission (2026-11-27)

- Common access admission resolves module policy without the local TTL cache.
- Read static membership/grant revision in a single RBAC-owner query before and after user policy resolution; reject changed authority (including membership ABA).
- Recheck current tenant/account/session or OAuth app, access registration and consent after asynchronous policy and clock work; never extend the original observed deadline or restore lost capabilities.
- Regression covers final admission after membership ABA and session revocation. Compilation, PostgreSQL contention, cross-replica runtime, exact atomic admission and downstream mutation fences remain open.

## Transactional user administration authority (2026-11-27)

- Built-in/custom create, update and delete independently revalidate the persisted active user actor and current module-filtered grants/hierarchy under user, module and ordered role locks. Preserve request permission/role ceilings and the existing last-active-SuperAdmin continuity guard. A service principal is not a user actor, even with a colliding subject ID.
- Materialize missing built-in destinations in the owner transaction before checking exact persisted grants; deny a missing requested role after the lock. Existing user locks are UUID ordered and deduplicated; static role locks cover actor and target memberships, destination and continuity role.
- Source regression covers stale request after demotion, disabled actor, service UUID collision and allowed built-in creation. Compilation, live PostgreSQL contention and other mutation paths remain open. This initial slice lacked proving session or OAuth registration evidence; the subsequent transaction-bound slices below extend it.

## Transaction-bound administrative credential authority

- Account deactivation now uses the shared rollback-only retry policy for
  typed RBAC generation reservation conflicts before durable event publication.
  A fresh attempt starts at request authorization and clock admission, repeats
  tenant/app/user/session or registration/consent/module/role fences and
  continuity checks, and publishes cache invalidation only after commit.
  Other deactivation statement errors and commit failures are not retried.
- Admin create retries typed account insertion/assignment errors; update
  retries typed generation reservation conflicts before event publication.
  Both reuse only validated inputs/hash, repeat clock and locked authority;
  update retains its original metadata baseline and rereads Flex values.
  Create refreshes Flex preparation before retry, not a schema-generation
  fence. No event/commit failure retry. GraphQL role owner SQL retries are
  wired separately in RBAC.
- Invite issuance retries typed ledger insertion conflicts before event
  publication; revocation retries ledger lock, receipt read and update
  conflicts. Acceptance retries receipt insertion/account creation conflicts
  with the original JWT, digest, pinned ledger and fresh clock/policy/issuer
  checks. Argon2 remains outside retries; abandoned tokens/receipts are never
  published. Acceptance and admin share an active-tenant fence before actor
  locks. Source regressions cover disabled tenant and synthetic SQLSTATE
  rejection. An opt-in PostgreSQL test (RUSTOK_AUTH_TEST_DATABASE_URL) checks
  that deactivation hits lock_timeout while the shared tenant fence is held,
  then a fresh admission denies the disabled tenant. Other statement errors,
  PostgreSQL credential revocation between
  attempts and end-to-end event/rollback verification remain open.

- All user-admin mutations now use the same pre-transaction clock admission
  and module-policy preparation as OAuth and RBAC control-plane writes.
  The active tenant row is checked and fenced for both direct and delegated
  user actors before users/credentials; PostgreSQL FOR SHARE prevents
  concurrent non-key changes to is_active, whereas KEY SHARE did not.
  SQLite retains a conditional write-before-read fence. The stale direct
  credential regression now also covers tenant deactivation with no user
  creation. Multi-connection PostgreSQL tenant-deactivation and lock-order
  evidence remain pending; no global deadlock guarantee is claimed.

- Source slice: the HTTP/WS host scope binds the admitted credential deadline,
  principal kind, session identity and account recovery revision. User-admin
  create/update/deactivate and invite issue/revoke lock the direct initiating
  session in user-before-session order and check expiry after policy locks
  and before commit. RBAC role assignment uses the same user port.
- Delegated user-admin source slice: admitted access registration ID, digest,
  client/app, original scopes, precise issuance and consent epoch are retained.
  Acquire the active tenant fence (PostgreSQL FOR SHARE or SQLite write-before-read), app, ordered users, registration,
  then existing invite/policy/role locks. Validate live registration and consent
  with the extractor's canonical scope checks. GraphQL HTTP and WebSocket
  initialization no longer replace credential evidence with a grant-only scope.
  Re-filter current role grants through the original OAuth scopes.
- OAuth administration source slice: all six admin mutations share the
  provider authority decision and caller-owned OAuth write transaction.
  The HTTP/WS scope retains exact service access registration evidence as
  well as delegated evidence. The active tenant fence precedes UUID-ordered
  issuer/target apps, then actor/session or access registration, module
  policy and role grants. Require original and live settings/delegation
  ceilings for app administration, exact direct session for personal consent,
  and the earliest signed/durable credential deadline before commit.
  Expensive secret hashing remains outside the transaction, with prepared
  rotation bound to app/tenant and a locked CAS. The outer guard no longer
  writes independently. Regression scenarios authored for revoked direct
  sessions and service registrations.
- RBAC control-plane source slice: create/update/delete role and the
  host artifact grant/revoke transport use the host credential authority
  port on their write transaction. Require direct session evidence,
  admitted and current management permissions, lock static module policy
  and actor/target role rows in UUID order, and check deadline immediately
  before commit (including artifact idempotent retries). Owner SQL fixtures
  explicitly inject a test-only authority adapter. Host/session and owner
  rollback regressions are authored, not executed.
- Typed Auth admin context source slice: CredentialAuthority distinguishes
  direct session, delegated registration, service registration and explicit
  internal intent. Host resolver binds the admitted facts to GraphQL User
  and OAuth admin and Native adapters; RBAC host bridge binds direct
  evidence at its port boundary. Exact equality with the currently valid
  host scope rejects substituted credentials and retains invalidation.
  Grant-only production mutations are denied; explicitly marked owner SQL
  unit fixtures retain test-only compatibility. A nested absent scope masks
  the outer request. No raw bearer token enters the port.
- Pending: remove remaining ambient request dependence where a fully
  explicit host capability can preserve invalidation; account for
  artifact-runtime authority and prove cross-module lock ordering (including
  tenant active state and all bulk credential revocation paths). Compile
  and PostgreSQL contention/revocation tests remain unrun.

- PostgreSQL credential FK parent admission (migration 000007): app tenant,
  token/code/consent tenant/app/user, translation app, deferred refresh-family
  root, invite issuer/consumption and email-change user references use
  nonblocking parent guards. Existing FK/lineage constraints and nullable
  cascades remain in place. Metadata-only mutations do not add earlier locks.
  PostgreSQL contention, successful retry and user cascade regression authored,
  unrun. Session refresh history, bulk parent deletes, all owner receipt/outbox
  and global cross-owner lock order remain separate open boundaries.

- SQLite physical user deletion parity: migration 000008 adds the missing
  token user cascade, deleting descendants before roots and preserving
  unrelated credentials. Existing orphan/cross-tenant rows block cutover;
  no automatic cleanup. Source tests authored, unrun. PostgreSQL existing
  composite FK is unchanged.

- Session/retention lock closure: exact session identity under the user fence
  for refresh/logout; platform migration 000010 for nonblocking session/history
  parent admission; PostgreSQL retention locks candidate apps before children,
  then exact roots/members and eligible code rows. Busy candidates abort the
  whole transaction; fixed conservative cutoff and SQLite behavior preserved.
  Source regressions unrun. Full session history immutability, tenant/user
  identity parity, bulk revocation and cross-owner/outbox ordering remain open.

- Session evidence integrity (platform migration 000011): immutable session
  identity/creation time, tenant/user composite FK or SQLite equivalent,
  append-only history and parent-only cascade, PostgreSQL TRUNCATE rejection
  and SQLite replacement guards. Dirty identity blocks migration. Source
  regressions and aging fixture adjustments authored; no runtime verification.
  Absolute deadline monotonicity, parent-delete authorization, bulk revocation
  and cross-owner/outbox ordering remain open.

- Broad OAuth user/app revocation: stable exact credential prelocks, NOWAIT
  before mutations, with existing owner fences and all-or-nothing rollback.
  App-wide scope includes consents; user scope preserves delegated-only tokens.
  PostgreSQL source contention test unrun. Family replay and consent paths,
  cross-owner receipts and outbox still require closure.

- Credential identity continuation: Auth migration 000014 adds append-only
  session, OAuth app/client/token/code UUID claims and session refresh/OAuth
  token/code digest claims. Current and retained historical session digests
  are backfilled; new session digest claims are atomic with birth/rotation. The SQLite witness table has no hidden rowid conflict key, and
  SQLite rejects alternate refresh-family REPLACE; normal rotation and
  rollback remain supported. Source regressions authored, unrun; incremental
  migration, lock contention and storage growth require verification. Tenant
  identity and terminal state transitions remain open only at other
  boundaries: sessions, OAuth tokens, codes and consents now reject changes
  to a recorded terminal timestamp. Revoked token rotation evidence and
  revoked consent grant/subject are immutable; consent UUIDs are permanent.
  Fresh consent epoch continuity and other nonterminal claim mutations
  remain open. Source regressions unrun.

- Transaction retry authority first consumer: consent revocation prelocks
  exact credential scope and preserves typed statement failures. Confirmed
  rollback and bounded Core retry re-enter clock and locked credential
  admission; commit errors never retry. Other admin and owner adapters,
  rollback/commit fault injection, full auth reauthorization contention
  and external-effect receipts remain open. Source regressions unrun.

- Consent writer correction: every grant now retires the existing active row
  and inserts a fresh UUID, invalidating exact-subject tokens and codes even
  when no active consent is present. The terminal guard must never be bypassed
  to revive a revoked row. Timestamp-based epoch binding remains a separate
  open cross-reader contract. Source regressions unrun.

- Issued OAuth claims: token scopes/expiry and code subject, redirect URI,
  scopes, PKCE and timestamps are immutable after issuance on both backends.
  Extractor negative fixtures now insert malformed legacy claims at birth
  instead of modifying an issued registration. Database regressions cover
  attempted live-claim mutation and no-op updates. Session deadline and
  durable consent epoch binding remain open. Source regressions unrun.

- Consent grant claims are immutable after birth, including active grants.
  Explicit session absolute deadlines only narrow; refresh materializes a
  legacy NULL once and otherwise preserves the stored cap. SQLite rejects
  ambiguous timestamp changes rather than allowing a possible extension.
  Timestamp-independent consent binding and sliding expiry remain open.
  Source regressions unrun.

- Authorization-code UUID generation: migration 000018 adds a write-once
  consent_id and the issuance/exchange path checks the exact live generation
  before preparation and under the app/subject lock. Legacy unbound codes
  for consent-required clients fail closed, without guessed backfill.
  Token registrations, refresh lineage and admin evidence UUID binding
  remain outstanding. Source regressions unrun.

- Consent-required token generation: resolve the immutable original
  authorization_code_id to the consumed code's consent UUID. Check exact
  tenant/client/subject and live generation in access, WebSocket, locked
  admin and refresh preflight/commit. Same-timestamp replacement regressions
  cover refresh and admin; unbound legacy families fail closed. Source
  regressions unrun.

- Consent generation admission checks active consent and consumed code in one
  SQL snapshot, and checks both prepared token models against the admitted
  original code and epoch before any exchange/rotation/recovery effect.
  Regression assertions cover a stale grant observation and mismatched
  prepared lineage. Source regressions unrun.

- OAuth tenant admission continuation: authorization-code issuance (including
  browser confirmation), direct consent grants, client-credentials exchange,
  code exchange and refresh rotation/recovery now reserve the active tenant
  before the app/subject fences. Direct consent grant entrypoints share strict
  validation and application-clock admission. Security-only replay revocations
  retain their independent app fence so inactive tenants cannot block cleanup.
  SQLite source regressions cover inactive/missing tenant rejection and absence
  of extra consent/code effects. A PostgreSQL multi-connection regression is
  authored for re-observation after an in-flight tenant deactivation commits;
  scoped compilation and all runtime evidence remain unrun. Global lock
  hierarchy is not closed.

- Cross-owner bulk revocation continuation: session bulk updates require an
  explicit owner transaction, prelock only live matching sessions in UUID
  order and exclude the proving session. Session and OAuth bulk revocation
  reserve PostgreSQL ROW EXCLUSIVE table modes with NOWAIT before row probes;
  a row-only NOWAIT does not cover implicit UPDATE relation locks. Authored
  PostgreSQL row/table contention, isolation and repeat-revocation regressions.
  No new retries or external effects; compilation and runtime evidence unrun.
  Administrative receipt/FK and full cross-owner ordering remain open.

- Exact OAuth lineage revocation continuation: RFC 7009, refresh replay and
  authorization-code replay reuse one caller-transaction token writer with
  identical scope for NOWAIT admission and UPDATE. PostgreSQL source regression
  covers exact row/table contention, unrelated-family isolation and repeat
  revocation. Protective replay remains independent of tenant active state.
  Compilation/runtime evidence unrun; no automatic replay of security effects.

- Administrative receipt/retention lock slice: PostgreSQL invitation receipt
  reservation and revocation use a nonwaiting global digest gate after role
  authority. A DB trigger fences ordinary receipt INSERT/UPDATE/DELETE on
  digest and UUID; ID, tenant and digest cannot change. Early replay probing
  reserves ACCESS SHARE NOWAIT; receipt writes reserve ROW EXCLUSIVE NOWAIT.
  Retention reserves explicit relation modes before candidate probes. Source
  regressions cover receipt unique-key and retention relation contention.
  SQLite writer serialization is unchanged. Cascade and direct SQL deletion
  authority remain open; compilation and runtime verification unrun.

- Session retention FK cascade continuation: pin exact expired session IDs in
  UUID order with FOR UPDATE NOWAIT, re-observe their history in a fresh READ
  COMMITTED statement and fence exact children before parent DELETE. Reserve
  both relation write modes NOWAIT. Keep the original cutoff, no extra round
  trip per session, and retain unrelated live history. PostgreSQL row/table
  contention and cascade rollback regressions authored, unrun. Other parent
  cascades and full lock hierarchy remain open.

- Invitation terminal authority: permanent issued/consumed digest witnesses
  are atomic with credential births and survive metadata retention and tenant
  removal. Immutable issuance claims and first revocation prevent rebinding or
  reactivation; receipt claims cannot be rewritten. Retention deletes ledger
  before receipt and skips receipts whose ledger remains. SQLite replacement
  and PostgreSQL truncate are rejected. DB-time deletion guard is supplemental
  to the verified owner clock; no plaintext or account identifiers in witnesses.
  Legacy pruned evidence/restore and other parent cascades remain open; source
  regression and compilation unrun.

- Pending terminal-authority migration refinement: explicit SQLite ABORT on
  malformed digest/kind prevents outer INSERT OR IGNORE from suppressing the
  inner witness constraint; digest-first witness primary key supports both
  phases. PostgreSQL issuance UUID/digest gates cover update/delete as well
  as birth. Added contention and mismatched receipt/ledger horizon regression
  sources; unrun. Pending migration amended in place per AGENTS.md §7.

- Auth physical parent cascade fence: PostgreSQL BEFORE DELETE on users,
  OAuth apps and sessions reserves exact Auth-child relation/row locks with
  NOWAIT, including nested session history. Active child RLS fails closed.
  Existing FK actions and witnesses remain authoritative; unrelated rows and
  service credentials survive user deletion. Source regression covers busy
  child rows/tables, rollback, retry, SET NULL and deferred family cascade;
  compilation and migration/runtime checks unrun. Tenant-wide and non-Auth
  cascades (including RBAC), outer parent lock admission and event coupling
  remain open. No readiness promotion.
