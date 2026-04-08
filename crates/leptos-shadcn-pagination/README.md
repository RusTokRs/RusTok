# leptos-shadcn-pagination

## Purpose

`leptos-shadcn-pagination` owns reusable pagination primitives for Leptos UI surfaces in RusToK.

## Responsibilities

- Provide pagination container and item primitives.
- Provide previous/next/link/ellipsis components with consistent semantics.
- Keep pagination markup reusable across admin and storefront UI packages.

## Entry points

- `Pagination`
- `PaginationContent`
- `PaginationItem`
- `PaginationLink`
- `PaginationPrevious`
- `PaginationNext`
- `PaginationEllipsis`

## Interactions

- Used by Leptos UI packages in `apps/admin`, `apps/storefront`, and module-owned Leptos surfaces.
- Complements higher-level table/list packages such as `leptos-table`.
- Stays presentation-focused and does not own data fetching or paging policy.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../docs/index.md)
