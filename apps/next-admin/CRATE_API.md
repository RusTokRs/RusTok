# apps/next-admin / CRATE_API

## Public Modules

- Next.js admin dashboard (App Router): layout, navigation, RBAC-aware pages, theme/clerk integrations.

## Key Structures/Contracts

- Public routes `/admin/*`.
- Contracts with backend API/GraphQL.
- Auth provider integration (Clerk).

## Events

- Publishes: admin commands to backend via API.
- Consumes: API responses and auth session events from Clerk.

## Dependencies on Other Crates/Packages

- `packages/leptos-*` (TS packages), backend `apps/server` API.

## Common AI Mistakes

- Confusing type sources between local `types` and generated GraphQL types.
- Breaking RBAC in sidebar/nav when refactoring routes.
