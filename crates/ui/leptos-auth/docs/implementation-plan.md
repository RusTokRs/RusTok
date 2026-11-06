# Implementation plan for `leptos-auth`

## Current state

`leptos-auth` is the shared Leptos authentication UI/runtime boundary for host
applications. It owns auth context, route guards, hooks, session storage, and
the package transport facade. Native server functions and the GraphQL selected
path live under `transport/`; the package does not own server auth policy.

The legacy `api` module is currently a compatibility re-export of `transport`.
It is not the target boundary and must be removed once all callers use the
canonical transport surface.

## FFA/FBA boundary

The 2026-11-06 Blog reference continuation exposed a WASM-only compile error in
`AuthorizedBrowserClient`: gloo `Headers::set` returns unit and throws, so it
cannot propagate a typed header failure. The client now obtains the underlying
`web_sys::Headers` and preserves its fallible `set` calls before request IO.
The existing credential source and request/auth ownership are unchanged.
Scoped compilation is recorded in the Blog plan; browser failure/session
execution remains pending and this correction does not promote readiness.

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `shared_ui_support`
- This shared UI support crate is not a module-owned FBA provider.

## Open results

1. **Remove the `api` compatibility re-export atomically.** Migrate every
   caller to `leptos_auth::transport` and delete the legacy namespace without
   retaining aliases or dual execution paths.
   **Depends on:** all shared auth UI consumers.
   **Done when:** no caller imports `leptos_auth::api`, `lib.rs` no longer
   exposes the re-export, and native/GraphQL behavior is unchanged.

2. **Prove native and GraphQL auth transport parity.** Cover sign-in, sign-up,
   sign-out, refresh, reset, and current-user success/rejection paths with the
   same canonical auth error mapping.
   **Depends on:** host server-function and GraphQL auth fixtures.
   **Done when:** selected transport behavior, session updates, tenant handling,
   and error classification are verified without parser or server-policy logic
   entering the package.

3. **Keep shared session/route behavior host-safe.** Evolve storage, hooks, and
   route guards only with explicit session lifecycle and host routing contracts.
   **Depends on:** a host auth/session requirement.
   **Done when:** session persistence, expiry, logout, and protected/guest route
   behavior remain consistent across Leptos hosts.

## Verification

- Targeted transport tests for all auth operations and error mapping.
- Host integration tests for session, tenant, and route-guard behavior.
- Source checks confirming no caller remains on the removed `api` namespace.

## Change rules

1. Keep server auth policy in `rustok-auth` and host/server adapters.
2. Keep native and GraphQL paths inside this package's transport boundary.
3. Update host auth documentation with a shared session or route contract change.
