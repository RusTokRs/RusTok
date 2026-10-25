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
