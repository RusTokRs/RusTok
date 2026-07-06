# apps/storefront / CRATE_API

## Public Modules
- Leptos storefront UI (SSR): catalog, product cards, content pages.

## Key Structures/Contracts
- Public storefront routes.
- GraphQL/HTTP contracts for content and catalog reads.

## Events
- Publishes: user read/search actions (via API requests).
- Consumes: `apps/server` responses, content/commerce data.

## Dependencies on Other Crates
- `leptos-ui`, `leptos-graphql`.

## Common AI Mistakes
- Using admin-oriented GraphQL operations in the storefront.
- Confusing SSR and CSR initialization points.
