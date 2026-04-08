# leptos-auth

## Purpose

`leptos-auth` owns the shared authentication UI/runtime boundary for Leptos-based RusToK applications.

## Responsibilities

- Provide auth context and route guards for Leptos hosts.
- Expose auth hooks and session/local-storage helpers.
- Keep native Leptos `#[server]` auth flows and GraphQL fallback on the same package boundary.

## Entry points

- `AuthProvider`
- `AuthContext`
- `ProtectedRoute`
- `GuestRoute`
- `RequireAuth`
- `use_auth`
- `api`

## Interactions

- Used by `apps/admin` and `apps/storefront` as the shared auth UI/runtime layer.
- Uses `leptos-graphql` for GraphQL fallback transport.
- Talks to `apps/server` auth endpoints and server-function surfaces without embedding server-specific policy in the UI package.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../docs/index.md)
