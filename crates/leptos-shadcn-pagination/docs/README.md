# `leptos-shadcn-pagination` Documentation

`leptos-shadcn-pagination` provides presentation-only Leptos pagination
primitives. It does not calculate pages, fetch data, or own route/query state.

## Public contract

- Container, content, item, and link components provide reusable pagination
  markup.
- Link active state uses `aria-current="page"`.
- Previous/next disabled state uses `aria-disabled` and presentation classes.
- Hosts supply pagination policy, href values, and localized previous/next
  content through the host effective locale.

## Boundary

Hosts and module UI packages own data fetching, page arithmetic, route/query
updates, and localization. Add behavior here only for reusable presentation and
accessibility semantics.

## Related documents

- [Crate README](../README.md)
- [Implementation plan](./implementation-plan.md)
