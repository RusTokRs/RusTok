# rustok-auth

## Purpose

`rustok-auth` owns authentication primitives for RusToK: password hashing, JWT lifecycle,
refresh-token helpers, invite-token helpers, auth config assembly/validation, and auth-related migrations.

The shared Leptos client lifecycle belongs to the Auth-owned [`rustok-auth-ui`](./ui/README.md)
FFA package and framework-neutral session models in `rustok-ui-auth`.

## Responsibilities

- Provide `AuthModule` metadata for the runtime registry.
- Own the shared browser/session FFA boundary in [`ui`](./ui/README.md), used by both hosts, and the admin pages in [`admin`](./admin/README.md).
- Expose auth primitives used by `apps/server` transport adapters and lifecycle services.
- Publish the typed `users:*` RBAC surface through `AUTH_USER_PERMISSIONS` and `RusToKModule::permissions()`.
- Own typed `AuthLifecyclePort`, `UserAdminMutationPort`, and `OAuthAdminPort` boundaries used by GraphQL and native adapters.
- Own the auth lifecycle and OAuth GraphQL query, mutation, input, and output surfaces behind the
  `graphql` feature. Its credential rate-limit extension uses shared `http` header types without a
  direct Axum dependency. It charges distinct credential mutation response keys in the selected
  operation and traverses root fragments without recursion or repeated expansion.
- Enforce tenant-composite integrity for OAuth apps, tokens, authorization codes,
  consents, and their user subjects at both database and query boundaries.
- Self-service email changes require current-password reauthentication, a short-lived
  one-time mailbox challenge, transactional identity/session updates, and a notice to
  the previous mailbox.
- Fail auth configuration fast for malformed or mismatched RS256 keys and keep
  OAuth code exchange and refresh rotation on one transactional execution path.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Used by `apps/server` for REST and GraphQL auth flows, session handling, and user lifecycle.
- Declares permissions via `rustok-core::Permission`.
- `apps/server` enforces those permissions through `RbacService`; `rustok-auth` itself does not depend on `rustok-rbac`.
- Human-readable RBAC ownership for the auth module is `users:*`.
- `apps/server` has registry and GraphQL security contract tests that compare host wiring against `AUTH_USER_PERMISSIONS`.

## OAuth application Translation boundary

OAuth application presentation copy is owner-localized in `oauth_app_translations` and is exposed to the neutral Translation control plane as `auth/oauth_application_copy`.

- Localizable owner copy is exactly `name` plus optional `description`.
- `slug`, `client_id`, secret material, icon/configuration, redirect URIs, scopes, grant types, delegated permissions, consent state, token state, usage timestamps, manifest provenance, and other operational/security fields stay outside Translation.
- Semantic resource and locale revisions hash only exact localized copy. Security/configuration-only mutations therefore do not invalidate Translation proposals or advance the owner copy cursor.
- Revoking/deactivating an OAuth application archives the Translation resource without changing its copy revision.
- Runtime authoring never uses the migration-only `und` locale. Legacy `und` rows remain storage provenance but are not exposed as source or target locales.
- Translation reads and applies retain the existing OAuth administration authority floor: `settings:manage`.
- OAuth application copy is treated as sensitive tenant data and is not eligible for AI export.
- The owner change journal is backfilled by the Auth migration and records create/copy/lifecycle changes for frozen ChangeCursor and aggregate progress evidence.
- `apps/server` owns the host adapter and exact-locale apply service; the Auth crate owns the storage contract, migration, semantic revision vocabulary, and lifecycle mapping.

## Entry points

- `AuthModule`
- `AUTH_USER_PERMISSIONS`
- `UserAdminMutationPort`
- `UserAdminMutationRuntime`
- `OAuthAdminPort`
- `OAuthAdminRuntime`
- `OAuthAppTranslationLifecycle`
- `oauth_app_translation_lifecycle`
- `oauth_app_translation_locale_revision`
- `oauth_app_translation_resource_revision`
- `AuthLifecyclePort`
- `AuthLifecycleRuntime`
- `graphql::{AuthQuery, AuthMutation, OAuthQuery, OAuthMutation}` (feature `graphql`)
- `AuthAdminMutationContext`
- `AuthAdminMutationError`
- `AuthConfig`
- `AuthSettingsOverrides`
- `build_auth_config_from_document`
- `validate_auth_config`
- `Claims`
- `encode_access_token`
- `decode_access_token`
- `encode_password_reset_token`
- `decode_password_reset_token`
- `encode_email_verification_token`
- `decode_email_verification_token`
- `encode_invite_token`
- `decode_invite_token`
- `generate_refresh_token`
- `hash_password`
- `verify_password`

## Docs

- [CRATE_API.md](CRATE_API.md)
- [Module docs](./docs/README.md)
- [Platform docs index](../../../docs/index.md)

## Current worker service contexts (2026-11-02; source only)

Auth now owns the narrow current authenticated read-context port used by the selected
[Inventory Index consumer](../rustok-distribution/docs/inventory-index.md). Its exact tenant/
consumer/correlation/budget request contains no credential, saved claims or event actor. The
server resolves canonical deployment SecretRef credentials and current OAuth service permissions;
it never provisions a service or adds grants. Human transport authentication is unchanged.

### Known Limitations / Pending Implementation

User-administration transactions (create/update/deactivate user and
issue/revoke invite, including RBAC role assignment through the user port)
carry the admitted HTTP/WS credential deadline and account recovery
revision in the host request scope. Direct session credentials lock the
initiating session after ordered user locks, validate the exact subject and
live absolute/sliding deadlines, and recheck time after policy/role locks
and before commit. Grant-only internal scopes do not claim credential
authority. Delegated user administration now carries the admitted registration
identity, digest, scopes, issuance time and consent epoch, locks the OAuth app
before ordered users, locks the registration, checks current app/consent policy
and restricts current grants to the original scopes. HTTP GraphQL and WS scope
construction preserve these facts. OAuth app create/update/rotate/revoke and
personal consent grant/revoke now run the host authority check and owner
write in the same transaction. Direct consent requires admitted direct
session evidence; service clients carry their exact access registration
and use live app grants intersected with admitted scopes. All involved app
rows are acquired in UUID order before user/session/registration and
policy/role fences. The outer OAuth guard performs preflight only, and the
secret preparation remains outside SQL transactions. RBAC role-definition
mutations and artifact grant/revoke REST handlers now invoke the same
direct-session authority fence on their RBAC-owned transaction, including
live module/static role permission checks and final credential expiry.
The Auth admin port now requires a typed CredentialAuthority (direct
session, delegated registration, service registration, or explicit
non-bearer internal intent). Host credential resolution binds the exact
admitted evidence to GraphQL and Native admin contexts and the RBAC
bridge; production authenticated ports reject internal/grant-only
authority. The host still corroborates the typed evidence with the
valid request scope to preserve invalidation and prevent substitution.
Explicit nested unauthenticated scopes mask outer credentials.
All five user-admin writes, OAuth admin writes and RBAC control-plane writes
now share pre-transaction application-clock admission. The active tenant is
fenced before app/user/credential locks, including direct user administration:
PostgreSQL uses FOR SHARE (not FK-only KEY SHARE), SQLite a conditional
write-before-read fence. The inactive-tenant regression is authored, unrun.
Artifact runtime HTTP/GraphQL dispatch, UI projection and audit now require
the admitted direct session, rechecking tenant, actor, session, current role
grants and credential deadline in a short host transaction. OAuth delegated
and service credentials are denied until an explicit dynamic permission
delegation contract exists. The shared runtime policy check also guards UI
reads and durable receipt replay. SQL locks are not held across sandbox
execution; policy/installation and external execution are not atomically
fenced by this slice. A claimed operation denied at recheck requires the
existing reconciliation flow.
The artifact-aware serving resolver is now source-wired from owner-selected
active installations and actual executor support, with exact node readiness
and policy-revision revalidation. Contextual policy and readiness producer
parity, post-admission runtime entitlement, remaining scope-independent owner
contract work and global PostgreSQL deadlock evidence remain open.
Compilation and runtime verification are pending.

OAuth authorization rechecks the locked direct-session/access deadline after
consent and code writes, and code consumption rejects future-dated and expired
codes using one admission timestamp. Service and delegated token exchanges
recheck prepared expiry before commit. Newly issued OAuth access rows use
the exact signed whole-second JWT expiry, and response TTL is computed after
usage telemetry. OAuth policy changes and consent re-grants now durably revoke previous
credentials at the source level. Compilation, focused regressions and
PostgreSQL contention/rollback evidence remain pending. OAuth refresh-family
proof-bound recovery is source-wired for the token endpoint; client adoption,
generated references and full OAuth lifecycle verification remain open. Newly issued
credentials are bounded by the original refresh-family deadline, and
rotation requires consistent consumed-root evidence and rejects multiple
live refresh branches. Pre-existing access registrations keep their stored
expiry. Canonical family identity, database uniqueness and family-wide retention are source-wired; migration execution and PostgreSQL verification remain pending.

Direct first-party refresh has source-level proof-bound recovery through the
raw and cookie REST endpoints and GraphQL HTTP transport. The cookie
REST owner issues an HttpOnly token/seed envelope and derives its proof;
Next Admin relays that envelope and direct Leptos CSR coordinates cooperating
tabs with Web Locks. Shared Leptos native/SSR BFF uses the raw proof scheme.
All use bounded ambiguous refresh retry. Standalone Storefront SSR composition,
Next Storefront, mobile/native proof handling, generated API header documentation, compilation
and multi-replica runtime evidence remain pending. See the
[proof-bound refresh ADR](../../../DECISIONS/2026-11-26-session-refresh-retry-proof.md).

JWT purpose keyrings, immutable prepared snapshots, fixed RS256 signing and
verification, exact key selection and retirement are source-implemented.
Mixed HS256/RS256 runtime branches, default key derivation, algorithm selectors
and the transitional keyring-document marker have been removed. Scoped
compilation, regression execution and coordinated replica rollout evidence
remain pending. Optional process-local file-backed hot reload, durable
generation fencing, identity tombstones, cluster lease/drain fencing,
rolling replica admission and emergency enrolled-key revocation are
source-implemented. The shared dev-only fixture generator supplies four
independent RSA pairs to Auth bootstrap, server special-token regressions,
durable fence, authority, witness, observability, local lease and immutable
snapshot rotation scenarios. Explicit isolated dev/test provisioning of four
independent RSA pairs is available through the
[keyring provisioning command](docs/README.md#isolated-development-and-test-provisioning).
File bootstrap constructs the selected complete key source directly without
instantiating default signing inputs. Candidate grant admission rejects local
policy mismatches and unmanaged publishers before durable grant mutation.
Do not reset non-disposable security state. See the
[cutover contract](docs/README.md#rs256-only-cutover).
All JWT purposes require an exact key ID; missing-ID access and raw-root
verification bridges are removed. Access grant type/scopes and invite nonce
are mandatory; previously accepted missing fields are rejected.
The keyring document uses one unversioned format with a bounded monotonic
generation and contains no algorithm field or alternate key source.
Password-reset credential binding and OAuth consent MAC proofs use one
unversioned canonical format. Zero-legacy cleanup remains open for other
unreleased token formats and broader config compatibility. Ambiguous or empty RSA key sources
are rejected.
The independent MAC root rejects PEM key material and weak values at JWT preparation, host-proof and durable fence derivation boundaries. Host root rotation remains open.
Clock discontinuity detection and dual-deadline leases are source-implemented;
browser code issuance carries the authenticated confirmation expiry through
locked consent and code writes and checks the shared live credential clock;
independent clock fault-injection and suspend evidence remain pending.
The Auth-owned retention operation also corroborates host/application DB time
at both transaction boundaries, retains the same session history for worker
and CLI, and refuses deletion on time uncertainty. OAuth credential mutations,
standalone consent grants, direct login/registration/refresh and the common access extractor now corroborate
application DB time with that same process-local guard; independent JWT
authority remains separate.
Independent production JWT authority and explicit offline import are source-implemented;
a separately configured rollback witness detects authority-only rollback and
supports exact offline recovery. Joint-store rollback and application session
rollback remain separate. See [JWT rotation](docs/README.md#jwt-keyring-and-rotation).
Operational JWT metrics and initial alerts are source-implemented; see
[operational observability](docs/README.md#operational-observability).
Compilation, metrics regressions and live scrape/alert evidence remain pending.


Context/configuration and revocation-double definitions are uncompiled/unrun. Actual canonical
OAuth/module availability/secret rotation, denied/revoked grants, backend and deployment evidence
remain pending; absent bindings refuse instead of falling back to system/admin authority.
The [existing downstream task record](../rustok-inventory/docs/implementation-plan.md) tracks
recovery/cutover gates. No public transport/UI or new operator capability is exposed here.

The shared Leptos CSR/hydrate provider now source-wires origin-wide transition
admission, foreign epoch invalidation and atomic verified memory publication.
See [Auth UI](ui/README.md); server-side expected-lineage admission is
source-wired for raw/cookie REST and GraphQL HTTP refresh, with mounted browser
and cross-replica verification pending.

## Recovery credential lifecycle (2026-11-26)

Administrative custom-role provisioning now validates both request-bound and
current persisted delegation under actor, module-policy and ordered role-grant
fences, then uses the same direct role creation path as invitation acceptance.
The target account has no temporary Customer membership; metadata, localized
values and response projection stay inside the transaction. Existing-account
role replacement retains its separate continuity checks. Built-in admin
provisioning still uses its previous request-authority contract. Source
regression unrun; PostgreSQL contention remains unverified.

Custom-role invitation acceptance inserts the account and assigns only the
already-authorized persisted target role inside the consumption transaction.
It does not bootstrap, assign or remove a temporary Customer membership, so
one new membership advances the membership revision once. The shared account
insertion preserves duplicate-email handling; the existing RBAC writer owns
assignment and continuity checks. Missing role or assignment failure rolls back
the account and receipt. Source regression unrun; admin provisioning is a
separate path, and PostgreSQL contention evidence remains pending.

Invite acceptance uses an unlocked ledger lookup only to locate the issuer,
then locks the issuer before the exact invitation and revalidates the digest,
account epoch and revocation under those fences. This avoids the inverse
issuer-delete/FK SET NULL lock order; committed consumption is rejected before
password hashing and again under the ledger fence before module/RBAC locks,
with the unique receipt insert retained as final authority.
SQLite starts with the user write fence. PostgreSQL contention is unverified.

Before invoking the email sender, the host re-reads the active account and
checks the JWT's exact recipient, tenant, user and recovery revision. Reset
also checks its password-hash MAC; already-verified accounts skip verification
mail. Email-change dispatch reads the exact tenant/user/target/digest challenge
and its owner in one statement, reuses the confirmation binding, and never
extends the committed expiry. No account locks are held during SMTP; a change
after the read can still make a link stale. Confirmation remains authoritative.
Source regressions are unrun.

All JWT issuers, including reset, verification and invite, sample the live
credential clock before preparing claims and recheck it after RSA signing.
Reject backward time or an elapsed exclusive signed whole-second expiry;
this does not extend the deadline or replace application-clock admission.
Source regression authored, unrun.

Email-change challenges bind the database-owned account recovery revision,
like reset and verification JWTs. Legacy pending email changes are removed
at migration; no revision backfill or compatibility reader is provided.
Recovery mutations recheck credential expiry before requesting commit and
roll back late transitions. Reset, verification and invite also repeat live JWT
validation after blocking work and at the final observation; invite issuance
revalidates its signed token before commit. Email-change confirmation and
background dispatch reject future-dated challenges; invite acceptance rejects
future-dated ledger rows. These are observed boundaries, not a guarantee of
validity at the database commit instant or durable clock safety. Source
regressions for future-dated credentials and early invite replay are unrun.
Verification records the corroborated post-lock timestamp. Email-change
confirmation uses the unique mailbox index as the conflict authority and a
savepoint to consume a conflicting challenge without changing the account.
Issuance reclaims invalid or expired reservations for the exact requested
mailbox under the old owner's mutation fence before locking the requester.
Live reservations and other tenants remain untouched.
See the [recovery revision ADR](../../../DECISIONS/2026-11-26-auth-recovery-revision.md)
and the [Auth implementation plan](./docs/implementation-plan.md) for remaining evidence.

## Durable application clock admission

Selected Auth credential and retention entrypoints persist corroborated host
and primary-DB high-water marks in `auth_application_clock` before domain
transactions. Later checks read the floor without a nested writer transaction.
Apply the Auth migration before deploying this version; never lower the floor.
Full durable expiry observation coverage, independent JWT authority continuity,
contention and restart/runtime evidence remain open. See the [clock ADR](../../../DECISIONS/2026-11-25-jwt-clock-discontinuity.md).

## Independent authority clock continuity

Witnessed authority reads and mutations now maintain an authenticated
DB-time floor in the existing witness payload, with synchronous commit
before exposing checked time, including a zero-row checked CAS without a
ledger revision change. Emergency revocation preserves it. Stop
old writers and drain leases before format cutover; never strip the floor.
Application late observations and request-path clock durability remain
open. See the [clock ADR](../../../DECISIONS/2026-11-25-jwt-clock-discontinuity.md).

## Independent application clock writer

Host and session-cleanup CLI wire a dedicated PostgreSQL clock writer pool.
Late verified samples commit independently of domain rollback; uncertain
commit outcomes fence the process. PostgreSQL routing identity is pinned
at startup and source rollback is checked before the independent write.
SQLite can use a separately provisioned, bound persistent clock file with
FULL/EXTRA sync and a persistent rollback journal or WAL, otherwise remains
admission-only when unbound. See the
[clock ADR](../../../DECISIONS/2026-11-25-jwt-clock-discontinuity.md) for
configuration, recovery and limits. Source implementation only; runtime,
contention and failover evidence remain pending.

Server artifact HTTP/command execution captures the original direct request
scope in a host-only per-execution lease, sharing the invalidation flag with
callback tasks. At initial lease admission and before each capability call,
the host rechecks the same tenant/user/session/recovery and current binding
permission through the existing transaction. Lease lifetime is bounded by the
original credential expiry and sandbox budget; it is removed on cancellation,
failure or completion. No bearer evidence crosses worker transport. This does
not revoke already-started external effects or provide an atomic fence for
every domain owner commit. Compilation and worker integration remain pending.

The server structured-data and event capability brokers now reuse the
transaction-bound direct credential and artifact permission read on their own
SQL effect transaction. Active tenant, actor, session and role grant locks
remain held through commit, and the locked credential deadline is checked
again immediately before committing. Lease membership and expiry remain
mandatory; no actor is inferred for owner work. Installation and module
policy concurrency fencing remains separate work.

PostgreSQL migration 000007 introduces nonblocking FK parent admission for
OAuth apps, token/code/consent rows, application translations, refresh-family
roots and invite/recovery children. Existing FK and immutable lineage guards
remain authoritative. Direct late writes reject busy parents (55P03) instead
of waiting; retry only from a new transaction. PostgreSQL runtime and broader
session/identity and cross-owner lock ordering remain unverified.

Migration 000008 closes the SQLite token/user deletion parity gap: physical
user deletion removes delegated descendants before roots, preserving service
and other-user credentials. Existing orphan/cross-tenant lineage blocks
migration for explicit reconciliation. Source tests unrun.

PostgreSQL retention uses ordered nonblocking candidate locks and app-first
OAuth family deletion with fresh whole-family expiry checks. Contention rolls
back the complete sweep; the next attempt starts a new transaction. SQLite
retains serialized writes. Source regressions are unrun.

The RBAC-owned migration 000013 retains permanent user/role/permission UUID
birth witnesses. A post-cutover hard-deleted user cannot be recreated under
the same UUID to revive old recovery or membership evidence. The witness
has no user FK or account PII and survives deletion. Pre-cutover deletions,
other credential IDs, privileged DDL and database restore are not covered.
See the RBAC owner documentation. Source regressions unrun.

### Permanent credential identity evidence

Auth migration 000014 preserves UUID birth claims for sessions, OAuth apps,
client IDs, token registrations (including family roots), and authorization
codes. Session refresh and OAuth token/code digests are consumed permanently:
hard deletion
and a fresh UUID cannot revive a previously registered digest. These claims
have no foreign keys, survive cleanup and cascade, and roll back with failed
births. PostgreSQL claims use nonwaiting per-key advisory fences; SQLite
rejects implicit replacement of a live refresh-family member and preserves
ordinary rotation after predecessor revocation. Token/code digest and client
identity updates are forbidden. Current and historical session digests are
backfilled without conflict suppression; every successful session birth or
changed digest claims a fresh identity in the same transaction. No-op updates
and rolled-back rotations consume nothing. The independent replay history
retains its existing grace-window and cleanup semantics.
The SQLite witness table uses WITHOUT ROWID so a hidden row identity cannot
be used to replace an unrelated consumed credential claim.
Witness storage grows with credential births; no plaintext is retained.
Pre-cutover removed identities, in-place terminal timestamp regression,
tenant identity and privileged DDL/restore are not
covered by this migration. Source regressions unrun.

Authenticated OAuth consent revocation prelocks exact token/code/consent
rows in the existing stable order. On a typed PostgreSQL statement conflict,
confirmed rollback permits a bounded fresh attempt with new clock admission
and locked credential authority. Commit failure is never auto-replayed.
Source regressions are unrun; other admin paths are not covered by this retry.
