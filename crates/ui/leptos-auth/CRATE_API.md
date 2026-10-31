# Crate: `leptos-auth`

Leptos authentication context, protected route wrappers, BFF cookie management, and reactive token refresh hooks.

## 1. Public API Surface

### Storage Keys & Constants
- `ADMIN_TOKEN_KEY`: Local/session storage key for authentication access token (`"rustok_admin_token"`).
- `ADMIN_TENANT_KEY`: Storage key for tenant slug context (`"rustok_admin_tenant"`).
- `ADMIN_USER_KEY`: Storage key for cached authenticated user details (`"rustok_admin_user"`).
- `ADMIN_SESSION_KEY`: Storage key for session metadata (`"rustok_admin_session"`).
- `ADMIN_REFRESH_COOKIE`: HttpOnly cookie name (`"rustok-admin-refresh-v1"`) for BFF session refresh.

### Cookie and Session Helpers
- `refresh_cookie_value(tenant: &str, refresh_token: &str) -> String`: Encodes refresh cookie value as `{tenant}.{refresh_token}`.
- `parse_refresh_cookie_value(value: &str) -> Option<(&str, &str)>`: Parses cookie value into tenant and token components.
- `extract_refresh_cookie(cookie_header: &str) -> Option<(String, String)>`: Extracts refresh credentials from HTTP Cookie headers.
- `refresh_cookie_header(tenant: &str, refresh_token: &str, secure: bool) -> String`: Constructs standard `Set-Cookie` header.
- `clear_refresh_cookie_header(secure: bool) -> String`: Constructs expired `Set-Cookie` header to invalidate session.
- `session_id_from_access_token(token: &str) -> Option<String>`: Unverified payload parser extracting stable session UUID for CSRF protection.

### Components
- `AuthProvider`: Top-level context provider managing authentication state and reactive signals.
- `RequireAuth`: Component wrapper requiring an authenticated session before rendering children.
- `ProtectedRoute`: Router wrapper protecting private route views.
- `GuestRoute`: Router wrapper restricting views to unauthenticated guests.

### Reactive Hooks
- `use_auth() -> AuthContext`: Retrieves the complete reactive authentication context.
- `use_current_user() -> Signal<Option<AuthUser>>`: Retrieves signal of the authenticated user.
- `use_is_authenticated() -> Signal<bool>`: Returns whether an active session is currently authenticated.
- `use_is_loading() -> Signal<bool>`: Returns whether session restoration or login is in progress.
- `use_is_token_valid() -> Signal<bool>`: Returns whether current access token is unexpired.
- `use_token() -> Signal<Option<String>>`: Returns current access token string.
- `use_tenant() -> Signal<Option<String>>`: Returns active tenant slug.
- `use_session() -> Signal<Option<AuthSession>>`: Returns active session structure.
- `use_auth_error() -> Signal<Option<AuthError>>`: Returns latest authentication error.

## 2. Invariants & Business Rules
- **BFF Cookie Isolation**: Refresh tokens are stored strictly in `HttpOnly; SameSite=Lax` cookies; browser client scripts cannot read refresh tokens directly.
- **SSR & Hydration Consistency**: Initial server auth snapshot (`ServerAuthSnapshot`) hydrates seamlessly into client reactive signals without auth flickering or premature redirection.
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
