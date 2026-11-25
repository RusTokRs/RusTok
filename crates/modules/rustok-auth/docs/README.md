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
be restarted. JWT signing and persisted session/refresh digests are unchanged.
Source regressions unrun. Deploy all host instances together.

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
