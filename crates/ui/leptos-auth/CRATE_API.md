# Crate: `leptos-auth`

Leptos authentication context, protected route wrappers, BFF cookie management, and reactive token refresh hooks.

### Constants
- `ADMIN_TOKEN_KEY`: retired credential-storage key, purged by the canonical browser reader; tokens are memory-only.
- `ADMIN_TENANT_KEY`: storage key for tenant slug context.
- `ADMIN_USER_KEY`: retired persisted-user key, purged on browser access.
- `ADMIN_SESSION_KEY`: retired persisted-session key, purged on browser access.
- `ADMIN_REFRESH_COOKIE`: HttpOnly cookie name (`rustok-admin-refresh-v1`) for BFF session refresh.

### Cookie and Session Helpers
- `refresh_cookie_value(tenant: &str, refresh_token: &str) -> String`: Encodes refresh cookie value as `{tenant}.{refresh_token}`.
- `parse_refresh_cookie_value(value: &str) -> Option<(&str, &str)>`: Parses cookie value into tenant and token components.
- `extract_refresh_cookie(cookie_header: &str) -> Option<(String, String)>`: Extracts refresh credentials from HTTP Cookie headers.
- `refresh_cookie_header(tenant: &str, refresh_token: &str, secure: bool) -> String`: Constructs standard `Set-Cookie` header.
- `clear_refresh_cookie_header(secure: bool) -> String`: Constructs expired `Set-Cookie` header to invalidate session.
- `session_id_from_access_token(token: &str) -> Option<String>`: Unverified payload parser extracting stable session UUID for CSRF protection.

### Components
- `AuthProvider(children, tenant_slug?)`: canonical context provider. An optional resolved host tenant binds only missing public tenant metadata; foreign saved tenants refuse.
- `RequireAuth`: component wrapper requiring an authenticated session before rendering children.
- `ProtectedRoute`: router wrapper protecting private route views.
- `GuestRoute`: router wrapper restricting views to unauthenticated guests.

### Reactive Hooks
- `use_auth() -> AuthContext`: retrieves the complete reactive authentication context.
- `use_current_user() -> Signal<Option<AuthUser>>`: retrieves signal of the authenticated user.
- `use_is_authenticated() -> Signal<bool>`: returns whether an active session is currently authenticated.
- `use_is_loading() -> Signal<bool>`: returns whether session restoration or login is in progress.
- `use_is_token_valid() -> Signal<bool>`: returns whether current access token is unexpired.
- `use_token() -> Signal<Option<String>>`: returns current access token string.
- `use_tenant() -> Signal<Option<String>>`: returns active tenant slug.
- `use_session() -> Signal<Option<AuthSession>>`: returns active session structure.
- `use_auth_error() -> Signal<Option<String>>`: returns latest authentication error.

## 2. Invariants & Business Rules
- **BFF Cookie Isolation**: Refresh tokens are stored strictly in `HttpOnly; SameSite=Lax` cookies; browser client scripts cannot read refresh tokens directly.
- **SSR ownership**: `ServerAuthSnapshot` is request-scoped input validated by the host, not a session inferred by this library. Browser restoration and SSR/hydration parity require mounted verification.
- **CSRF Protection**: Stable session ID derived from unverified access token payload serves as request secret across state-modifying requests.
- **Tenant Context Preservation**: Tenant identity is strictly isolated and retained across session refreshes.

## 3. Error Handling & Edge Cases
- Malformed cookies or tampered tokens fail closed returning `None` without panicking.
- Network and token refresh failures cleanly update reactive `AuthError` signal.
- Expired tokens seamlessly trigger refresh attempts prior to GraphQL transport queries.

## 4. Security & Isolation Invariants
- `HttpOnly; SameSite=Lax` cookie flags enforced in `refresh_cookie_header`.
- `Secure` cookie attribute dynamically toggled based on deployment environment (HTTPS).
- Plain request-scoped server snapshot prevents cross-request data leaks in multi-tenant SSR hosts.

## 5. Contract Tests & Verification
- Unit test suite: 3 tests verifying GraphQL transport URL shapes, authenticated client wire types, and server snapshot scoping.
- `src/contract_tests.rs`: Direct verification of package metadata, cookie serialization/deserialization invariants, and session ID extraction.
