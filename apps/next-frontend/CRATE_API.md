# apps/next-frontend / CRATE_API

## Public Modules
- Next.js storefront (App Router): public catalog/content/search pages.

## Key Structures/Contracts
- Public storefront routes.
- Data read contracts from backend API/GraphQL.

## Events
- Publishes: client query requests and user actions to backend.
- Consumes: API responses and cached client state.

## Dependencies on Other Crates/Packages
- `packages/leptos-graphql` and related frontend packages, backend `apps/server`.

## Common AI Mistakes
- Moving admin-specific contracts/components into the frontend storefront.
- Import errors between server/client components in Next.js.
