# `leptos-table` Documentation

`leptos-table` provides shared, serializable table-state DTOs for Leptos admin
and operator surfaces. It is independent of concrete data sources, routes, and
domain query contracts.

## Public contract

- `TableState` holds page, page size, sort rules, and filter rules.
- `SortRule` and `SortDirection` describe transport-friendly sorting.
- `FilterRule` describes a field/value filter without interpreting domain
  semantics.

## Boundary

Hosts and module-owned UI packages own route/query state, data fetching,
validation, and domain-specific filters. Add behavior here only when it is
reused across independent table surfaces.

## Related documents

- [Crate README](../README.md)
- [Implementation plan](./implementation-plan.md)
