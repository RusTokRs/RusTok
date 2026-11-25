# `rustok-auth` Documentation

`OAuthAppTranslationLifecycle` serializes as `active` or `archived`, matching
its canonical persisted spelling. Host translation snapshots consume this owner
type directly rather than defining another lifecycle enum.

`rustok-auth` is the core authentication module of the platform. It holds JWT lifecycle,
The `AuthLifecycleContext` carries the trusted optional `AuthPrincipalKind` classification; lifecycle self-service operations that depend on a direct user session must reject delegated OAuth and service principals rather than deriving principal kind from transport fields.
credential hashing, refresh/reset/invite/email-verification token flows and
runtime RBAC surface `users:*`.

Auth lifecycle GraphQL (`AuthQuery`, `AuthMutation`) and OAuth GraphQL (`OAuthQuery`,
`OAuthMutation`) are owner-owned by `rustok-auth` behind the `graphql` feature. `apps/server`
only implements `AuthLifecyclePort`, `UserAdminMutationPort`, and `OAuthAdminPort` providers over
the persisted lifecycle/OAuth/email services and registers the corresponding runtimes.
Auth, OAuth and users REST request/response DTOs for login, registration, refresh/logout,
invite/reset, email verification, profile/password, sessions, user list/detail, token,
authorize/consent, browser-session and revocation flows live in `rustok-auth::rest`; the server
controller modules re-export or import those owner DTOs only for OpenAPI/route compatibility.

## Purpose

- keep auth domain logic outside `apps/server`;
- publish the canonical runtime entry type `AuthModule`;
- provide the platform with a unified contract for tokens, claims and credential helpers.

## Scope

- auth configuration, JWT algorithms and host-provided override assembly/validation;
- encode/decode helpers for access/reset/invite/email-verification token flows; JWT encoders reject TTL values that cannot be represented safely as a checked `chrono` duration;
- password hashing, password verification and refresh-token helpers; refresh-token generation uses the OS RNG and propagates RNG failures as a typed `AuthError::RefreshTokenGenerationFailed` instead of panicking;
- auth-owned migrations;
- auth-owned auth/OAuth/users REST DTO/OpenAPI schema surface in `rest.rs`, with host controllers
  limited to transport extraction, persistence adapters and response mapping;
- publication of permission surface `users:*` via `AUTH_USER_PERMISSIONS` and `RusToKModule::permissions()`.
- typed application boundaries `UserAdminMutationPort` and `OAuthAdminPort` for admin commands, OAuth reads and consent lifecycle without module crate dependency on host transport;
- `AuthUserBackfillDbReader` implements a tenant-scoped identity projection with a strict `1..=500` batch bound, checked SQL-limit conversion, deterministic `created_at,id` ordering, and redacted storage/row-read errors for owner-owned profile backfill.
- `AuthUserBootstrapDbWriter` owns tenant-scoped idempotent identity provisioning for installer workflows; email lookup is case-normalized, supported backends are `Postgres`/`Sqlite`, unsupported backends fail closed without panics, and storage/row-decoding failures are exposed only as stable owner errors. Caller-owned transactions remain supported for atomic identity+RBAC installation.
- owner-owned OAuth GraphQL query/mutation/types behind `graphql` feature; `apps/server` only implements the runtime port over the DB and connects roots into the common schema;
- browser OAuth authorization is a host transport flow: cookie-authenticated GET requests are side-effect-free and always render a short-lived HMAC-CSRF confirmation form, even when prior consent exists or the app type does not require consent. Authorization codes are issued only after the CSRF-validated POST; explicit non-ambient Authorization-header requests may retain the pre-authorized fast path.

OAuth persistence is tenant-composite rather than a set of independent foreign
keys. PostgreSQL requires each token, authorization code, and consent to reference
an app and optional/required user in the same tenant; invite-consumption user
links enforce the same rule while retaining their delete-to-null audit semantics.
SQLite uses equivalent
insert/update and parent-tenant-move triggers for local and test profiles. The
forward migration rejects pre-existing cross-tenant relations instead of deleting
security evidence. Server consent reads and revocations include `tenant_id` in the
database predicate as an independent application-layer boundary.

OAuth credential tables deliberately do not use PostgreSQL tenant RLS because the
unauthenticated OAuth protocol must resolve an application from globally unique
`client_id` before a tenant context exists. That exception is limited to OAuth
protocol persistence: tenant-composite database constraints, tenant-qualified
application reads after resolution, active-app checks, grants, scopes, consent,
user state, and session state remain fail-closed boundaries.

The HTTP token endpoint calls the hardened `OAuthTokenService` directly. Authorization
codes and refresh tokens are consumed with compare-and-set updates in the same
transaction that persists a replacement refresh token. The former controller-local
and `OAuthAppService` token issuance paths have been deleted, so router middleware
ordering cannot re-enable a non-transactional replay path.

Invite acceptance and administrative revocation serialize on the same
`auth_invites` issuance row. PostgreSQL/MySQL use a locking read; SQLite acquires
its write fence before rereading the row. A revocation committed first makes
acceptance fail as invalid, an acceptance committed first cannot be retroactively
revoked, and repeated revocation does not emit duplicate lifecycle events. The
unique `auth_invite_consumptions.token_hash` constraint remains the independent
single-use replay fence.

The host additionally composition-gates the OAuth protocol surface and discovery metadata with
`settings.rustok.features.oauth_enabled`; when the gate is closed the routes are not mounted and
OpenAPI omits them. This gate does not replace the per-request active-app, consent, scope, and
credential-epoch checks below.

RFC 7009 revocation is a host transport adapter over a transactional, family-scoped
service operation: the token hash is resolved inside the transaction, `family_id`
coalesces to the row id for pre-lineage rows, and the whole lineage is revoked with
one client/tenant/user-scoped update. Unknown hashes succeed without revealing
existence. Client authentication for revocation uses the security-only active-app
lookup, not presentation translation hydration. OAuth access JWTs are registered
credentials: the signed `jti` names a durable `oauth_tokens` access row, and the
host extractor fails closed unless that row is active and hash/client/tenant/grant/
subject-bound. RFC 7009 and security-family revocation revoke access and refresh
rows together, fencing already-issued access JWTs before their signed TTL.
Delegated rows also capture the consent epoch so revoke/re-grant cannot resurrect
older credentials.

The current inline `oauth_apps.name` and `oauth_apps.description` fields do not yet
satisfy the platform multilingual storage contract. Their translation-table cutover
is an explicit release-plan priority in this module's implementation plan; transports
must not introduce a competing JSON or request-local fallback in the meantime.

## Purpose-separated host MACs

Auth derives distinct 32-byte keys with HKDF-SHA256 for OAuth browser confirmation
and password-reset credential binding. Host adapters use `auth_purpose_mac` rather
than raw shared-secret hashing or local HMAC. MAC v5 and reset fingerprint v2
replace earlier formats without fallback; outstanding forms and reset links must
be restarted. Persisted session/refresh digests are unchanged.
Source regressions unrun. Deploy all host instances together.


## JWT keyring and rotation

Auth uses four independent rings: access (direct and OAuth), password reset,
email verification, and invite. Each has one active key and bounded verification
history. Special JWTs require a purpose-specific `typ` and `kid`; access retains
the existing `JWT` type. The caller selects the ring before inspecting the header. Host startup eagerly
prepares all four rings as one immutable snapshot; clones share both the raw
key inputs and the prepared result. Direct
constructors prepare on first use. Key configuration changes detach the candidate
before mutation; invalid candidates never replace the prior snapshot. Only Auth-emitted JOSE
fields are accepted; unknown fields, duplicate IDs and explicit null IDs fail
closed even during the missing-ID migration.

Unknown fields in `settings.auth` fail configuration deserialization rather than
silently falling back to JWT defaults. Correct misspelled or obsolete override
names before startup.

Set `settings.auth.jwt_keyring_env` to the environment variable holding access
ring JSON. Access uses the existing `algorithm` and RSA settings. Set
`settings.auth.jwt_special_keyrings_env` to a separate environment variable holding
an object with optional `password_reset`, `email_verification` and `invite`
entries. Each entry has `algorithm` (HS256 by default), `keyring`, and, for RS256,
`rsa_private_key_pem` plus `rsa_public_key_pem`. Do not commit secret JSON.

Each ring object accepts:

- `active_kid`: 1–64 ASCII letters, digits, hyphens or underscores.
- `active_hs256_secret`: optional independent signing secret for HS256.
  If omitted, HKDF-SHA256 derives an independent purpose key from the host
  MAC root. The raw root is never an active JWT key.
- `verification_keys`: up to 16 objects with `kid`, `material` (actual HS256
  signing secret or RSA public PEM), and exclusive Unix-second `verify_until`.
- `legacy_without_kid`: access-only bounded migration setting with `kid` and
  `verify_until`. Special rings reject this setting.

The unconfigured access ID is `default`; special IDs are `reset-default`,
`verify-default`, and `invite-default`. Example special configuration shape
(use independent high-entropy secret material supplied by the secret manager):

```json
{
  "password_reset": {
    "algorithm": "HS256",
    "keyring": {
      "active_kid": "reset-next",
      "active_hs256_secret": "<independent-secret>"
    }
  }
}
```

RSA keys must parse as actual public/private RSA material with a modulus of at
least 2048 bits. The signing boundary checks that the private key matches the
configured public key; isolation uses canonical RSA moduli, not PEM formatting.
All explicit symmetric material is subject to production secret policy even
with RSA access. Snapshot preparation rejects key material reuse across rings before any key can
be used. Derived
keys isolate disclosure of a derived signing key, not compromise of the
common root; use independent explicit material for stronger isolation.

### Cutover and rotation

For previous default HS256 access deployments, configure a new active ID
and put the old raw host secret under the old ID `default` in access
`verification_keys`, with a cutoff covering the remaining access JWT lifetime.
If necessary, configure `legacy_without_kid` to point to that old entry.
Existing independent access signing material or RSA access pairs can retain
their IDs. The old raw host secret is not allowed as an active signing key.
Old shared-key special links are intentionally rejected and must be reissued.
Keep the host MAC root unchanged during this migration; refresh sessions are
not rewritten or revoked. Coordinate all replicas for this cutover.

For subsequent same-algorithm rotations, distribute the next verification
material to all replicas first, then promote it and retain the outgoing
material until its JWTs expire. For derived HS256 keys, the historical
material is the actual lowercase hex-encoded HKDF output (not the root).
Prefer explicit independent secrets for operational rotations. Remove
expired history; never reuse IDs or key material. The exclusive cutoff is
checked both during selection and after signature verification. HS256 verification material
is cryptographically capable of signing; use RSA for distribution to
untrusted verifiers. Startup environment configuration remains supported. An optional
versioned file source supports process-local hot reload (below). Auth request paths reuse
prepared encoding/decoding keys and indexed verification history; the JWT library
still constructs its internal crypto providers per operation.
A source regression now exercises staging, promotion, retirement and exact
key selection using two independent RS256 key pairs, including OAuth access and
bounded missing-ID migration. It is not yet executed. Algorithm migration and
multi-replica execution evidence are pending.
See [purpose keyring ADR](../../../../DECISIONS/2026-11-25-jwt-purpose-keyrings.md).

### Process-local hot reload

Set `RUSTOK_JWT_KEYRING_FILE` to a trusted secret-manager-mounted JSON file.
Do not combine this source with startup JWT algorithm, RSA or keyring overrides.
The file is limited to 256 KiB and requires a positive generation and a complete
set of four purpose entries. Example shape (replace placeholders with actual
independent high-entropy material or use the documented derived defaults):

```json
{
  "schema_version": 1,
  "generation": 1,
  "access": {"algorithm": "HS256", "keyring": {"active_kid": "access-1"}},
  "password_reset": {"algorithm": "HS256", "keyring": {"active_kid": "reset-1"}},
  "email_verification": {"algorithm": "HS256", "keyring": {"active_kid": "verify-1"}},
  "invite": {"algorithm": "HS256", "keyring": {"active_kid": "invite-1"}}
}
```

For RS256 entries provide the matching private and public PEM fields alongside
the keyring. Every update is a complete snapshot, not a patch. Replace the
mounted file atomically and increase `generation`. The host checks every five
seconds, validates all four rings and production secret policy before publication,
then replaces the generation and prepared keys together under a write fence.
Existing AuthConfig clones share the live source. Synchronous signing and
verification hold a read fence; no fence is held through database or network IO.
Failed reads, invalid documents, stale generations and incompatible successor
keys retain the last-known-good snapshot. A rejected file revision is retried
when its contents change. Never reuse an ID or key material: durable admission enforces identity
continuity, purpose separation and terminal removal. The root MAC secret and issuer/audience/TTL/password policy are not
rotated here.

### Durable generation admission

Apply the Auth-owned migration before starting the host (including static
configuration mode). The database singleton authenticates canonical key metadata
and lifetime key ID/material bindings with keys derived from the stable host
MAC root. It does not store signing keys, raw HMAC secrets or PEMs. A candidate
must be admitted durably before a local publication, including startup. An
identical canonical generation can restart; older or conflicting generations
cannot. Removing a key ID or material is terminal, even if a later document
renames the material or places it in another purpose. An active key moved to
history cannot sign again; newly staged verification keys can be promoted once
before expiry. Initial history is conservatively verification-only; enroll
before staging a future signer. Retained deadlines can only shorten; a removed
missing-ID mapping cannot be enabled again. The 8192-identity ledger limit
fails closed rather than forgetting retired identities.

The first file enrollment prevents subsequent static-mode startup against
the same database. Keep the MAC root and database authority stable; do not
restore an older database or run an older host binary. Stop old writers before
enrollment. An unavailable database or unknown publication result terminates
the host during admission; a crash after durable commit but before local
publication can recover by restarting with the committed generation. A
rejected candidate retains the local snapshot.

This is not a replica barrier: unchanged files and already-running replicas
are not checked against the database per JWT operation. Multi-replica readiness
and emergency distributed revocation remain open. In-flight business
transactions that already authenticated are not cancelled by publication.
Keys removed before first enrollment are not known to the ledger.
Root rotation, database rollback protection and trusted-clock rollback
protection are outside this contract. Source regressions are authored, unrun.

See [durable generation fence ADR](../../../../DECISIONS/2026-11-25-jwt-generation-fence.md).

## JWT principal and invite admission

Access JWT encoders and decoders enforce the canonical `AuthPrincipalKind`
classification. Direct JWTs require a non-nil session and no OAuth scopes or
registration ID. Delegated and service JWTs require non-nil client and durable
registration IDs, a nil session and an admitted OAuth access grant. The
`refresh_token` exchange still issues an `authorization_code` access grant.
Signature admission does not replace the host's live session or OAuth row
checks. Invite issuance and verification share the same custom-role predicate:
custom roles require the Customer baseline and a nonblank slug of at most 64
bytes. New invitation JWTs carry a random non-nil `jti` so independent issuances
with identical claims and timestamps cannot collide in the unique token-digest
ledger. Previously issued invitations without a `jti` remain admissible subject
to the same persisted exact-digest and one-use checks. Source regressions are
not yet executed.

## JWT and browser confirmation hardening

Both HS256 and RS256 host configuration require at least 32 non-whitespace
bytes of symmetric secret because browser confirmation and credential binding
still use it. JWT validation enforces `iat > 0`, `iat <= now`, `iat < exp`,
and `nbf` when present; checked duration construction rejects out-of-range TTL.
OAuth browser confirmation MAC v4 includes the direct session ID and rejects
expiries beyond its ten-minute window. Source regressions unrun.

## OAuth revocation client fence

RFC 7009 revocation rechecks the authenticated client snapshot under the app
mutation lock before inspecting the token. Concurrent secret or client policy
changes invalidate the preflight authentication; unknown tokens still succeed
for an authenticated client. Source regression unrun.

## OAuth issuance fences

Browser approval and authorization-code issuance share one transaction. The host
locks app, user and direct session in order, rechecks ownership, session liveness,
email policy and access credential expiry, then grants missing consent only for
explicit approval. Code insertion failure rolls back the consent change.

Re-granting consent invalidates pending authorization codes for the same app,
subject and tenant. Exchange rejects codes older than current consent and
rechecks the refresh grant under the app lock before persisting prepared refresh
credentials. OAuth refresh replay grace requires a nonnegative elapsed duration
less than 30 seconds. Source regression definitions are unrun.

## Direct session concurrency

The host uses user-before-session locking for direct refresh and bulk self-service
revocation, matching credential changes. Refresh revalidates the digest and subject
under the locks; historical replay locks the session only and never acquires a user
lock afterward. Bulk self-service revocation requires a still-active current session.
SQLite history-only replay acquires the writer before reading the history.

## Direct session refresh history

The host records each rotated refresh digest and its timestamp in `session_refresh_history`
inside the refresh transaction. The session remains the tenant and revocation authority;
all historical digests are rejected, and replay outside the 30-second per-token grace
revokes a live session. The previous-token columns are a last-rotation projection for
logout with a stale cookie, not the replay authority. History cascades on session deletion.
See [the ADR](../../../../DECISIONS/2026-11-25-session-refresh-history.md).

## GraphQL credential throttling

The Auth-owned rate-limit extension charges each distinct root credential mutation
response key in the selected operation. Different aliases are separate charges;
merged selections of the same field are not. Root fragment traversal is iterative
and each named fragment is visited once per operation, even before schema validation.
The normal GraphQL validator still rejects invalid documents. REST and GraphQL
share an IP-only Auth namespace budget; authenticated password change and logout
also consume it. Tenant, actor and mutation names do not partition the budget.
Backend-failure semantics remain unchanged.

## Responsibility Zone

`rustok-auth` owns authentication, OAuth, credential, token, and user-lifecycle
contracts. Host applications own transport extraction, persistence adapters,
and composition, but must not duplicate auth policy or token semantics.

## Integration

- depends only on `rustok-core` and common libraries, without dependency on `rustok-rbac`;
- used by `apps/server` for REST, GraphQL, session lifecycle and user-management flow;
- `apps/server/src/controllers/auth.rs` remains an HTTP adapter over auth-owned DTOs and lifecycle ports;
- `apps/server` checks registry wiring and GraphQL security hints against `AUTH_USER_PERMISSIONS`, so the host layer does not diverge from the auth-owned permission surface;
- `apps/server` implements ports on top of existing auth lifecycle/OAuth services and registers providers in shared runtime extensions; GraphQL and native `#[server]` adapters must consume one provider per boundary;
- publishes admin pages via `crates/modules/rustok-auth/admin` with `ui_classification = "admin_only"`; the separate [`ui` client capability](../ui/README.md) supplies one shared session boundary to both Leptos hosts without adding a storefront route;
- email delivery and transport wiring remain the responsibility of the host layer and adjacent modules.

## Config Lifecycle Surface

The canonical `AuthConfig` assembly is performed via `build_auth_config` /
`build_auth_config_with_env`: the host passes framework configuration, and
`rustok-auth` applies defaults, `AuthSettingsOverrides`, RS256 env key
resolution and validation. `apps/server` must not duplicate these rules, but
only map `AuthError` to a transport-specific error type.

For RS256, validation parses both PEM values and signs and verifies a probe at
startup. Malformed or mismatched key pairs therefore fail configuration assembly
before the server accepts traffic.

## Token Lifecycle Surface

The canonical set of auth-owned token helpers:

- access tokens: `encode_access_token`, `decode_access_token`;
- OAuth access tokens: `encode_oauth_access_token`;
- password reset tokens: `encode_password_reset_token`, `decode_password_reset_token`;
- email verification tokens: `encode_email_verification_token`, `decode_email_verification_token`;
- invite tokens: `encode_invite_token`, `decode_invite_token`.

The server password-reset consumer accepts only the credential-bound path. The
token subject includes a keyed fingerprint of the password hash present at
issuance, confirmation compare-and-swaps that exact hash, and all sessions are
revoked in the same transaction. There is no unbound reset execution fallback;
replay after the first successful password change fails closed.

Special-purpose tokens contain a strict `purpose` claim, use common JWT validation
`issuer`/`audience` and normalize email-subject to lowercase before issuance.
The host layer (`apps/server`) must publish transport endpoints only through these
helpers, so that invite/reset/verification flows remain auth-owned.

## Runtime Permission Set

The canonical set of permissions owned by the auth module:

- `users:create`
- `users:read`
- `users:update`
- `users:delete`
- `users:list`
- `users:manage`

When adding, removing or renaming permissions, update `AUTH_USER_PERMISSIONS`, `AuthModule::permissions()`, server registry/security tests and this document in a single increment.

## Incident Response

Primary owner for auth/JWT/RBAC incidents — Platform security/auth on-call. Escalation path: owner of `crates/modules/rustok-auth`, then owner of the server API surface.

On auth degradation:

1. Check `/health/ready`, `email_backend` and recent auth/API errors without logging secrets, reset/invite tokens or refresh tokens.
2. Verify effective `AuthConfig`: algorithm/key pairing, issuer, audience, TTL bounds and production policy.
3. If the issue relates to email reset/verification delivery, escalate also to the owner of host email transport.
4. If the issue relates to RBAC, verify `AUTH_USER_PERMISSIONS`, server registry/security hints and actual transport guards.
5. After rollback preserve evidence: artifact id, config snapshot without secrets, affected flows, health snapshot and list of revoked/rotated credentials if rotation was performed.

## Verification

- `cargo xtask module validate auth`
- `cargo xtask module test auth`
- targeted server tests for auth/RBAC contracts when changing runtime wiring

## Related Documentation

- [README crate](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Manifest Layer Contract](../../../../docs/modules/manifest.md)
