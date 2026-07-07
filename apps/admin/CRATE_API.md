# apps/admin / CRATE_API

## Public Modules
- Leptos admin UI: catalog, content, and user management pages.
- Client layers for GraphQL/auth/forms on `leptos-*` crates.

## Key Structures/Contracts
- Public admin UI routes.
- GraphQL query/mutation contracts to `apps/server`.

## Events
- Publishes: user actions via HTTP/GraphQL (entity creation/modification).
- Consumes: API responses and auth states.

## Dependencies on Other Crates
- `leptos-auth`, `rustok-graphql`, `leptos-hook-form`, `leptos-table`, `leptos-ui`.

## Common AI Mistakes
- Incorrect imports between `leptos-*` crates (confusing packages/ and crates/ variants).
- Breaking RBAC guard at the navigation level.
