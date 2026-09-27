# HttpOnly session migration for Leptos browser authentication

- Date: 2026-09-27
- Status: Accepted
- Owner: authentication/UI transport architecture

## Problem

The standalone Leptos admin compatibility bridge currently mirrors the browser's serialized authentication session from LocalStorage into same-origin cookies so SSR can construct a request-scoped auth snapshot. Those cookies are not HttpOnly because browser JavaScript cannot create an HttpOnly cookie.

This keeps compatibility with the existing browser transport, but it leaves access and refresh credentials reachable from JavaScript and therefore within the blast radius of an XSS vulnerability.

## Decision

The target browser authentication contract is a server-issued session cookie with:

- HttpOnly
- Secure in production
- SameSite=Lax (or stricter where a host explicitly permits it)
- explicit Path
- server-side rotation/invalidation
- CSRF protection for cookie-authenticated state-changing requests

The migration MUST be performed across the shared Leptos auth storage/transport contract and every affected admin/storefront browser API path. A partial migration that leaves bearer tokens in LocalStorage or mixes independently trusted cookie and LocalStorage identities is not considered complete.

## Security invariant

No SSR identity, role, tenant or authorization decision may be derived from unsigned/unenforced browser storage. Browser-provided session material is only a transport envelope until the canonical auth service validates it.

## Scope

The implementation belongs to the shared Leptos/browser transport work and is therefore tracked for FS-13 (shared frontend/browser packages) after the current FS-11 SSR hardening. FS-11 itself removes the forged-cookie SSR authorization path immediately but does not claim the broader HttpOnly migration complete.

## Required completion evidence

- no access/refresh credential persisted in LocalStorage;
- server-issued HttpOnly session cookie used by SSR and browser requests;
- explicit CSRF policy for cookie-authenticated mutations;
- refresh/logout/session revocation parity across native and GraphQL transports;
- all auth consumers migrated away from the legacy LocalStorage bearer transport;
- maintainer-run browser/integration tests recorded in the phase ledger.
