# leptos-table

## Purpose

`leptos-table` owns shared table-state contracts for Leptos admin and operator UI surfaces in RusToK.

## Responsibilities

- Represent paging, sorting, and filtering state in a transport-friendly shape.
- Provide reusable DTOs for table-oriented UI composition.
- Keep generic table-state logic out of domain-specific pages.

## Entry points

- `TableState`
- `SortRule`
- `SortDirection`
- `FilterRule`

## Interactions

- Used by Leptos admin and module-owned UI packages that render paged/sorted lists.
- Complements UI primitives such as `leptos-shadcn-pagination`.
- Stays independent from concrete data sources and domain-specific query contracts.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../docs/index.md)
