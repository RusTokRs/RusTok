# Implementation plan for `leptos-table`

## Current state

`leptos-table` owns shared, serializable table-state DTOs for Leptos admin and
operator surfaces: `TableState`, `SortRule`, `SortDirection`, and `FilterRule`.
It represents paging, sorting, and filtering without a data source, transport,
route, or domain query dependency.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `shared_ui_support`
- This UI support crate is not a module-owned FBA provider.

## Open results

1. **Validate the shared table-state contract with consumers.** Confirm how
   host and module packages serialize, normalize, and apply paging, sorting,
   and filters before expanding the DTO surface.
   **Depends on:** at least one concrete admin/operator table consumer.
   **Done when:** consumer tests prove the same transport-friendly state shape
   without domain-specific query rules in this crate.

2. **Add focused DTO contract tests.** Cover construction, serde roundtrip, sort
   direction, multi-sort ordering, filter preservation, and pagination edge
   cases as consumers rely on them.
   **Depends on:** the agreed shared table semantics.
   **Done when:** changes to public DTOs fail a compact, focused test suite.

3. **Keep the crate generic.** Extract additional table behavior only when it
   is reused across independent surfaces; keep route/query, data fetching, and
   domain filters with the host or owner module.
   **Depends on:** demonstrated cross-surface duplication.
   **Done when:** the public API remains transport-friendly and has no
   framework-specific policy beyond shared table state.

## Verification

- Targeted unit tests for DTO construction and serde roundtrips.
- Consumer tests in the adopting host/module UI package.

## Change rules

1. Do not add concrete data-source, route/query, or domain-filter logic here.
2. Update the local README with a changed public table-state contract.
3. Update consumers when a serialized DTO shape changes.
