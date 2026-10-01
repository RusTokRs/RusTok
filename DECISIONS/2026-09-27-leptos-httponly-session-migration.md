# HttpOnly session migration for Leptos browser authentication

- Date: 2026-09-27
- Decision status: Accepted
- Implementation status: In progress
- Owners: authentication/UI transport architecture
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

The standalone Leptos admin compatibility bridge mirrors serialized authentication state from LocalStorage into JavaScript-readable cookies so SSR can construct a request-scoped auth snapshot.

Those cookies cannot be HttpOnly because browser JavaScript creates them. The bridge therefore leaves access and refresh credentials reachable from JavaScript.

## Decision

The target browser authentication contract is a server-issued session cookie with:

- HttpOnly
- Secure in production
- SameSite=Lax (or stricter where explicitly required)
- explicit Path
- server-side rotation and invalidation
- explicit CSRF protection for cookie-authenticated state-changing requests

The migration covers the shared Leptos/browser transport and all affected admin/storefront consumers. A partial migration that leaves bearer credentials in LocalStorage or mixes independently trusted cookie and LocalStorage identities is not complete.

## Sources of truth and ownership

- Canonical session state and credentials owner: `rustok-auth`.
- Frontend session storage and transport adapters: `leptos-auth` and `apps/admin`.

## Invariants

No SSR identity, role, tenant or authorization decision may be derived from unsigned browser storage. Browser-provided session material is only a transport envelope until canonical auth verification succeeds.

### Allowed states

- Authenticated session with valid HttpOnly cookie verified server-side.
- Anonymous/unauthenticated visitor without session credentials.

### Forbidden states

- Deriving authentication or tenant role from untrusted LocalStorage.
- Client-visible access or refresh tokens in DOM/LocalStorage.

## Non-goals

- Altering the backend OAuth2/JWT token signing infrastructure.
- Re-architecting non-browser headless API token authentication.

## Data, transaction, and concurrency boundary

Session cookies and server-side session revocations are managed within `rustok-auth`. State mutations are protected by CSRF tokens.

## Context dimensions

Tenant, principal, roles, and session expiry participate in cookie-based authentication verification.

## Events and projections

Session revocation and logout events clear server-side session state and invalidate client cookies.

## Failure semantics

Invalid, expired, or missing session cookies fail closed to unauthenticated visitor context.

## Migration and cutover

Phased rollout from client-side LocalStorage cookie mirroring to server-issued HttpOnly session cookies across Leptos and Next.js admin surfaces.

## Alternatives considered

- Retaining LocalStorage token storage: rejected due to credential exposure to client scripts.
- Pure memory-only tokens: rejected because page reloads would lose authentication state.

## Verification

End-to-end browser tests verify that tokens are not exposed to JavaScript and session cookies carry HttpOnly/SameSite attributes.

## Consequences

Eliminates XSS-based credential theft via JavaScript storage inspection. Requires server-side session management and CSRF protection for cookie-based browser mutations.
