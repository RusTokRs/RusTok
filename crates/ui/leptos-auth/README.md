# leptos-auth

For full public interface and contract definitions, see [CRATE_API.md](CRATE_API.md).

## Purpose

`leptos-auth` owns the shared authentication UI/runtime boundary for Leptos-based RusToK applications.

## Responsibilities

- Provide auth context and route guards for Leptos hosts.
- Expose auth hooks and session/local-storage helpers.
- Keep native Leptos `#[server]` auth flows and GraphQL selected path on the same package boundary.

## Entry points

- `AuthProvider`
- `AuthContext`
- `ProtectedRoute`
- `GuestRoute`
- `RequireAuth`
- `use_auth`
- `transport`
- `api` compatibility re-export for existing callers

## Interactions

- Used by `apps/admin` and `apps/storefront` as the shared auth UI/runtime layer.
- Uses `rustok-graphql` for GraphQL selected-path transport.
- Talks to `apps/server` auth endpoints and server-function surfaces without embedding server-specific policy in the UI package.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../../docs/index.md)

## Host-resolved public tenant binding

`AuthProvider` accepts optional `tenant_slug`. Before browser silent refresh, it calls
`storage::bind_request_tenant`: a missing public tenant may populate the existing canonical
`ADMIN_TENANT_KEY`, but a foreign existing tenant/session is never overwritten. Failed binding
refuses restoration rather than silently borrowing a default tenant. It issues no credentials,
creates no auth storage key/protocol and does not invent an SSR session. Browser native headers
still read canonical auth storage; header application failures now refuse before IO. SSR hosts
must validate and supply their own auth snapshot where required; native Product reads use the
validated Axum request context. Browser header mutation uses the fallible
`web_sys::Headers` obtained from the Leptos request wrapper. The gloo convenience
setter returns unit and throws; it cannot implement this error contract.

## Client auth scope and outstanding verification

The provider exposes a non-secret generation and marks it unavailable during transitions. In
source, cooperating providers in the same browser document use a shared lease and refuse overlap;
a dropped future poisons that document's lease because it does not cancel a request or its cookie
side effects. Notifications contain no credentials. If notification storage is unavailable, a
transition fails before network I/O. Other tabs receive an asynchronous invalidation stamp, not an
atomic lock, and old requests may still affect cookies after a reload. Server authentication and
tenant checks remain authoritative. These changes need locked native/WASM and mounted-provider
verification; standalone state-machine tests do not prove the browser protocol. The Blog reference
plan records earlier bounded package/WASM compilation, not validation of this change.
