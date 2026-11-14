# rustok-ui-auth / CRATE_API

## Purpose

`rustok-ui-auth` defines framework-neutral client authentication contracts, session lifecycle data models, token expiration evaluation, and client-facing error abstractions shared across all RusToK UI adapters (Leptos, Dioxus, Next.js/React).

## Architecture & Responsibilities

- **Client Session Contract**: Defines `AuthUser` and in-memory `AuthSession` representing the client-side session state.
- **Credential Protection**: Implements custom redacted `Debug` formatting for `AuthSession` so short-lived access tokens are never leaked to logs or diagnostic outputs.
- **Expiry Evaluation**: Provides host-relative timestamp evaluation for remaining token lifetime (`secs_until_expiry`) and refresh window safety checks (`is_expired`).
- **Error Mapping**: Provides client-facing `AuthError` enum with status code mapping via `AuthError::from_status`.
- **Stateless & Framework-Agnostic**: Zero dependencies on UI framework runtimes (Leptos, Dioxus, React), routing, browser storage, or GraphQL transports.

## Public Surface

### Primary Data Types
- `AuthUser`: Authenticated user profile representation:
  - `pub id: String`
  - `pub email: String`
  - `pub name: Option<String>`
  - `pub role: String`
- `AuthSession`: Short-lived browser access token session:
  - `pub token: String`
  - `pub expires_at: i64`
  - `pub tenant: String`
- `AuthError`: Client authentication error enumeration:
  - `Unauthorized`: Missing or invalid session on protected resource.
  - `InvalidCredentials`: Authentication failed during login (401).
  - `Network`: Network connectivity failure.
  - `Http(u16)`: Other HTTP status code failures.

### Methods & Helpers
- `AuthSession::secs_until_expiry(&self, now: i64) -> i64`: Calculates remaining session lifetime relative to given host Unix timestamp with saturation.
- `AuthSession::is_expired(&self, now: i64) -> bool`: Checks if session is within the 60-second client-side refresh safety margin (`now >= expires_at - 60`).
- `AuthError::from_status(status: u16, is_login: bool) -> AuthError`: Maps HTTP response status codes to domain client errors.
- `std::fmt::Debug for AuthSession`: Custom debug implementation ensuring `token: "<redacted>"`.

## Invariants & Guarantees

1. **Token Redaction**: `AuthSession` debug formatting MUST ALWAYS redact `token` to `"<redacted>"` to prevent credential exfiltration via log aggregation.
2. **Refresh Token Absence**: Refresh tokens MUST NEVER be stored in `AuthSession`; they are managed strictly via HttpOnly cookies by server or BFF adapters.
3. **Saturating Expiry Math**: Expiration calculations use `saturating_sub` preventing panic or integer underflow under adverse clock skews.
4. **Safety Margin**: A session is considered expired if less than 60 seconds remain before `expires_at`, allowing proactive background refresh before token invalidation.
5. **Zero Framework Runtime Imports**: The crate must remain strictly UI framework agnostic and not import Leptos, Dioxus, or React runtimes.

## Dependencies & Boundaries

- **Upstream Dependencies**:
  - `serde`: Serialization and deserialization contracts for session data.
  - `thiserror`: Typed client-side error derivation.
- **Downstream Consumers**:
  - `rustok-auth-ui`: Leptos reactive context and routing guards.
  - `rustok-ui-dioxus`: Dioxus authentication hooks and state management.
  - Storefront and Admin UI client packages.
- **Boundary Rules**:
  - Server authentication, token issuance, password hashing, and RBAC authorization remain owned by backend crates (`rustok-auth`, `rustok-rbac`).
  - No network transport, cookies, or browser storage dependencies.
