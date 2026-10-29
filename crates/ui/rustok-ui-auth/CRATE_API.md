# rustok-ui-auth / CRATE_API

## Public surface

### Primary Data Types
- `AuthUser`: authenticated user metadata (`id`, `email`, `name`, `role`).
- `AuthSession`: in-memory short-lived access token session container (`token`, `expires_at`, `tenant`).
- `AuthError`: client-facing authentication error enum (`Unauthorized`, `InvalidCredentials`, `Network`, `Http(u16)`).

### Methods & Helpers
- `AuthSession::secs_until_expiry(&self, now: i64) -> i64`: calculates remaining session lifetime relative to given Unix timestamp.
- `AuthSession::is_expired(&self, now: i64) -> bool`: checks if session is within the 60-second client-side refresh safety margin.
- `AuthError::from_status(status: u16, is_login: bool) -> AuthError`: maps HTTP response status codes to client-facing auth errors.
- `Debug for AuthSession`: custom `Debug` implementation redacting access tokens (`token: "<redacted>"`).

## Invariants

- **Token Redaction**: `AuthSession` debug formatting must always redact the access token to prevent accidental credential leakage in diagnostic logs.
- **Refresh Token Omission**: Refresh tokens must never be stored in `AuthSession`; they are managed solely via HttpOnly cookies by server or BFF adapters.
- **Framework Independence**: Must not depend on Leptos, Dioxus, React, router, cookie, or GraphQL runtimes.
