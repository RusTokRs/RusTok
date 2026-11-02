# rustok-auth / CRATE_API

## Public Modules

`admin_mutations`, `backfill`, `bootstrap`, `config`, `credentials`, `error`, `graphql`, `jwt`, `lifecycle`, `migrations`, `module`, `oauth_app_translation`, `rest`.

## Platform Capability Bindings

- **Core Module Registration**: Exposes `AuthModule` implementing `RusToKModule` (slug `"auth"`, kind `ModuleKind::Core`) and `MigrationSource`.
- **Token & JWT Lifecycle**:
  - `encode_access_token`, `decode_access_token`
  - `encode_invite_token`, `decode_invite_token`
  - `encode_password_reset_token`, `decode_password_reset_token`
  - `encode_email_verification_token`, `decode_email_verification_token`
  - `encode_oauth_access_token`
- **Credential Management**:
  - `hash_password`, `verify_password` (Argon2)
  - `generate_refresh_token`, `hash_refresh_token`
- **Administrative & Runtime Ports**:
  - `UserAdminMutationPort` / `UserAdminMutationRuntime`
  - `OAuthAdminPort` / `OAuthAdminRuntime`
  - `AuthLifecyclePort` / `AuthLifecycleRuntime`
  - `AuthUserBackfillReadPort` / `AuthUserBackfillRuntime`
  - `AuthUserBootstrapDbWriter`, `AuthUserBackfillDbReader`
- **Configuration**: `AuthConfig`, `build_auth_config`, `validate_auth_config`.

## Domain Collaborations

- **RBAC & Identity**: Publishes canonical `AUTH_USER_PERMISSIONS` (`users:create`, `users:read`, `users:update`, `users:delete`, `users:list`, `users:manage`).
- **Database Migrations**: Applies migration history for users, credentials, sessions, refresh tokens, and OAuth application registry.
- **REST & GraphQL Adapters**: Provides serializable request/response types for auth endpoints in `apps/server`.

## Primary Public Types and Signatures

- `pub struct AuthModule`
- `pub struct AuthConfig`
- `pub enum AuthError`
- `pub struct Claims`
- `pub struct UserAdminMutationRuntime`
- `pub struct OAuthAdminRuntime`

## Dependencies on Other RusToK Crates

- `rustok-api`
- `rustok-core`
- `rustok-telemetry` (optional feature `graphql`)

## Common AI Mistakes

- Generates raw JWT tokens without checking tenant claims or expiration.
- Uses plaintext passwords or outdated hashing instead of canonical Argon2 helpers.
- Circumvents `UserAdminMutationPort` by updating database credential tables directly.

## Minimum Contract Set

### Input DTOs/Commands

- `CreateUserCommand`, `UpdateUserCommand`
- `CreateOAuthAppCommand`, `UpdateOAuthAppCommand`
- `LoginParams`, `RegisterParams`, `ChangePasswordParams`, `RequestResetParams`, `ConfirmResetParams`
- `TokenRequest`, `RefreshRequest`, `RevokeRequest`
- All changes to public DTO fields require synchronized updates in host transports.

### Domain Invariants

- Password hashing must use Argon2 with salt and configured cost parameters.
- Multi-tenant tenant boundary checks are mandatory on all user lookups and mutations.
- Refresh tokens must be hashed before persistence; plaintext refresh tokens are returned exactly once upon generation.
- Expired or revoked tokens must fail verification closed.

### Events / Outbox Side Effects

- State changes (user registration, password changes, token revocation) emit domain events for audit and session termination.

### Errors / Failure Codes

- `AuthError` and port-specific mutation errors (`AuthAdminMutationError`, `AuthLifecycleMutationError`) represent typed failure reasons (InvalidCredentials, TokenExpired, UserNotFound, ValidationFailed).

## Current worker service contexts

`WorkerServiceContextPort` / `WorkerServiceContextRuntime` resolve one bounded, exact-tenant
read context for a real selected consumer. The port belongs to Auth, not Index or Inventory.
Its request carries tenant, consumer, correlation and a read budget, never an event actor,
saved claim set, invented service identity or authorization receipt. Returned contexts must
retain exact scope, a non-nil UUID service actor, bounded deadline and read semantics.

The server adapter resolves a deployment-owned `SecretRef` using `rustok-secrets` and calls
the same current access-token/principal resolver as authenticated transports. Only OAuth
`client_credentials` service principals are admitted; current app grants and module availability
restrict claims. The host projects current authenticated facts into canonical `AuthContext` and
uses `AuthContext::port_actor()`: service actors carry the genuine authenticated `client_id`,
which is distinct from the separately verified application/JWT subject. That distinction is not
an identity defect and the worker must not introduce a competing subject-based actor rule.
Secret resolution uses `resolve_fresh_for_tenant`, which enforces the canonical tenant policy
without consulting or populating the shared secret cache; invalidate-then-resolve is not an
atomic freshness guarantee. No permission is added by this port. Credential material and full contexts
are not persisted in jobs, events or runtime-extension metadata and are never logged.
Absent configuration is unavailable, not a system/admin fallback. This source contract has no
new HTTP/GraphQL authentication protocol and does not auto-create a service principal.

The actual selected consumer uses this port as `inventory-stock-primary`. Host bindings are
`RUSTOK_WORKER_SERVICE_TOKENS_JSON` (at most 256 bindings / 256 KiB), containing exact canonical
SecretRef DTOs only, never nested inline tokens. The optional mounted resolver uses
`RUSTOK_WORKER_SERVICE_SECRET_MOUNT_ROOT`. Each read budget is 1 ms–30 seconds; token material is
at most 16 KiB. Configuration validation and context tests are uncompiled/unrun; actual OAuth,
module-availability, secret rotation/revocation and deployment evidence remain pending.
