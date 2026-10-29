# leptos-auth / CRATE_API

## Public surface

### Constants
- `ADMIN_TOKEN_KEY`: local/session storage key for authentication access token.
- `ADMIN_TENANT_KEY`: storage key for tenant slug context.
- `ADMIN_USER_KEY`: storage key for cached authenticated user details.
- `ADMIN_SESSION_KEY`: storage key for session metadata.
- `ADMIN_REFRESH_COOKIE`: HttpOnly cookie name (`rustok-admin-refresh-v1`) for BFF session refresh.

### Cookie and Session Helpers
- `refresh_cookie_value(tenant: &str, refresh_token: &str) -> String`: encodes refresh cookie value as `{tenant}.{refresh_token}`.
- `parse_refresh_cookie_value(value: &str) -> Option<(&str, &str)>`: parses cookie value into tenant and token components.
- `extract_refresh_cookie(cookie_header: &str) -> Option<(String, String)>`: extracts refresh credentials from HTTP Cookie headers.
- `refresh_cookie_header(tenant: &str, refresh_token: &str, secure: bool) -> String`: constructs standard `Set-Cookie` header.
- `clear_refresh_cookie_header(secure: bool) -> String`: constructs expired `Set-Cookie` header to invalidate session.
- `session_id_from_access_token(token: &str) -> Option<String>`: unverified payload parser extracting stable session UUID for CSRF protection.

### Components
- `AuthProvider`: top-level context provider managing authentication state.
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
- `use_auth_error() -> Signal<Option<AuthError>>`: returns latest authentication error.

## Invariants

- **BFF Cookie Isolation**: Refresh tokens are stored strictly in `HttpOnly; SameSite=Lax` cookies; browser client scripts cannot read refresh tokens directly.
- **SSR & Hydration Consistency**: Initial server auth snapshot (`ServerAuthSnapshot`) hydrates seamlessly into client reactive signals without auth flickering or premature redirection.
- **CSRF Protection**: Stable session ID derived from unverified access token payload serves as request secret across state-modifying requests.
