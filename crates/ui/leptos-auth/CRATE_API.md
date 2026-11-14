# Crate: `leptos-auth`

Leptos authentication context, protected route wrappers, BFF cookie management, and reactive token refresh hooks.

### Constants
- `ADMIN_TOKEN_KEY`: retired credential-storage key, purged by the canonical browser reader; tokens are memory-only.
- `ADMIN_TENANT_KEY`: storage key for tenant slug context.
- `ADMIN_USER_KEY`: retired persisted-user key, purged on browser access.
- `ADMIN_SESSION_KEY`: retired persisted-session key, purged on browser access.
- `ADMIN_REFRESH_COOKIE`: HttpOnly cookie name (`rustok-admin-refresh-v1`) for BFF session refresh.
- `AUTH_SCOPE_CHANGE_EVENT`, `AUTH_SCOPE_CHANGE_STORAGE_KEY`: non-secret lifecycle invalidations; neither is authorization or a cross-tab lock.

### Cookie and Session Helpers
- `refresh_cookie_value(tenant: &str, refresh_token: &str) -> String`: Encodes refresh cookie value as `{tenant}.{refresh_token}`.
- `parse_refresh_cookie_value(value: &str) -> Option<(&str, &str)>`: Parses cookie value into tenant and token components.
- `inspect_refresh_cookie_headers(headers) -> Result<Option<(String, String)>, RefreshCookieError>`: Inspects all Cookie headers; `Ok(None)` means absent, while malformed and duplicate credentials are errors.
- `refresh_cookie_header(tenant: &str, refresh_token: &str, secure: bool) -> String`: Constructs standard `Set-Cookie` header.
- `clear_refresh_cookie_header(secure: bool) -> String`: Constructs expired `Set-Cookie` header to invalidate session.
- `session_id_from_access_token(token: &str) -> Option<String>`: Unverified payload parser extracting a session identifier; it is not authentication or a CSRF secret.

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
- **Client scope**: `ClientAuthScope` and `AuthContext::client_scope` expose a non-secret, fail-closed transition generation. Cooperating providers share an in-document operation lease; it cannot coordinate other tabs or cancel an already-sent request. Owner/server authentication and CSRF checks remain authoritative.
- **Tenant Context Preservation**: Tenant identity is strictly isolated and retained across session refreshes.

## 3. Error Handling & Edge Cases
- Native refresh returns an explicit absence only when no BFF refresh cookie exists. Malformed, duplicate, foreign-tenant, rejected and transport-failed credentials leave the client scope unverified; the direct-cookie 401 does not distinguish absence from rejection.
- `transport::refresh_session` returns `Result<Option<(AuthSession, AuthUser)>, AuthError>`; `None` is the explicit native absence, not a failed credential.
- Native sign-out preserves the cookie if revocation cannot be confirmed. The raw logout endpoint rejects already-invalid tokens, so idempotent native cleanup and visible failure recovery still need verification.
- Browser refresh failures and the mounted notification/lease behavior need integration verification; a failed transition is not a verified guest.

## 4. Security & Isolation Invariants
- `HttpOnly; SameSite=Lax` cookie flags enforced in `refresh_cookie_header`.
- `Secure` cookie attribute dynamically toggled based on deployment environment (HTTPS).
- Plain request-scoped server snapshot prevents cross-request data leaks in multi-tenant SSR hosts.

## 5. Contract Tests & Verification
- `src/scope.rs`: standalone lifecycle state-machine tests; they do not exercise the provider or browser lease.
- `src/contract_tests.rs`: package metadata, cookie serialization/deserialization and session ID extraction.
- The current browser-provider, cross-provider and network/cookie behavior still requires mounted verification.
