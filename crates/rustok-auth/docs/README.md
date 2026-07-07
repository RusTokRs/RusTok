# `rustok-auth` Documentation

`rustok-auth` is the core authentication module of the platform. It holds JWT lifecycle,
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

## Area of Responsibility

- auth configuration, JWT algorithms and host-provided override assembly/validation;
- encode/decode helpers for access/reset/invite/email-verification token flows;
- password hashing, verify and refresh-token helpers;
- auth-owned migrations;
- auth-owned auth/OAuth/users REST DTO/OpenAPI schema surface in `rest.rs`, with host controllers
  limited to transport extraction, persistence adapters and response mapping;
- publication of permission surface `users:*` via `AUTH_USER_PERMISSIONS` and `RusToKModule::permissions()`.
- typed application boundaries `UserAdminMutationPort` and `OAuthAdminPort` for admin commands, OAuth reads and consent lifecycle without module crate dependency on host transport;
- owner-owned OAuth GraphQL query/mutation/types behind `graphql` feature; `apps/server` only implements the runtime port over the DB and connects roots into the common schema.

## Integration

- depends only on `rustok-core` and common libraries, without dependency on `rustok-rbac`;
- used by `apps/server` for REST, GraphQL, session lifecycle and user-management flow;
- `apps/server/src/controllers/auth.rs` remains an HTTP adapter over auth-owned DTOs and lifecycle ports;
- `apps/server` checks registry wiring and GraphQL security hints against `AUTH_USER_PERMISSIONS`, so the host layer does not diverge from the auth-owned permission surface;
- `apps/server` implements ports on top of existing auth lifecycle/OAuth services and registers providers in shared runtime extensions; GraphQL and native `#[server]` adapters must consume one provider per boundary;
- publishes its own UI via the sub-package `crates/rustok-auth/admin` with `ui_classification = "admin_only"`;
- email delivery and transport wiring remain the responsibility of the host layer and adjacent modules.

## Config Lifecycle Surface

The canonical `AuthConfig` assembly is performed via `build_auth_config` /
`build_auth_config_with_env`: the host passes Loco/other framework config, and
`rustok-auth` applies defaults, `AuthSettingsOverrides`, RS256 env key
resolution and validation. `apps/server` must not duplicate these rules, but
only map `AuthError` to a transport-specific error type.

## Token Lifecycle Surface

The canonical set of auth-owned token helpers:

- access tokens: `encode_access_token`, `decode_access_token`;
- OAuth access tokens: `encode_oauth_access_token`;
- password reset tokens: `encode_password_reset_token`, `decode_password_reset_token`;
- email verification tokens: `encode_email_verification_token`, `decode_email_verification_token`;
- invite tokens: `encode_invite_token`, `decode_invite_token`.

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

Primary owner for auth/JWT/RBAC incidents — Platform security/auth on-call. Escalation path: owner of `crates/rustok-auth`, then owner of the server API surface.

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

## Related Documents

- [README crate](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Manifest Layer Contract](../../../docs/modules/manifest.md)
