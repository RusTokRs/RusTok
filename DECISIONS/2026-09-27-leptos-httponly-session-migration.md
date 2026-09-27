# HttpOnly session migration for Leptos browser authentication

- Date: 2026-09-27
- Status: Accepted
- Owner: authentication/UI transport architecture

## Problem

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

The migration must cover the shared Leptos/browser transport and all affected admin/storefront consumers. A partial migration that leaves bearer credentials in LocalStorage or mixes independently trusted cookie and LocalStorage identities is not complete.

## Security invariant

No SSR identity, role, tenant or authorization decision may be derived from unsigned browser storage. Browser-provided session material is only a transport envelope until canonical auth verification succeeds.

## Scope

The full migration is tracked for FS-13 shared frontend/browser packages. FS-11 fixes the immediate SSR identity-spoofing boundary but does not claim the broader HttpOnly migration complete.

## Completion evidence

- no access/refresh credential persisted in LocalStorage;
- server-issued HttpOnly session cookie used by SSR and browser requests;
- explicit CSRF policy for cookie-authenticated mutations;
- refresh/logout/session revocation parity across transports;
- all auth consumers migrated away from the legacy LocalStorage bearer transport;
- maintainer-run browser/integration verification recorded in the ledger.


## Next.js Admin browser boundary

The Next.js admin currently copies the RusTok access bearer into the NextAuth session object (session.user.rustokToken) so client components can read it via useSession. This makes the backend bearer reachable to browser JavaScript and is the same class of credential exposure the server-issued HttpOnly target is intended to eliminate.

The shared browser-auth migration in FS-13 MUST remove the backend bearer from the client-visible session shape and route client API calls through a server-owned transport/proxy or another server-issued session mechanism. The client-visible session may contain display-safe identity and UI state, but never a reusable backend access or refresh credential.
