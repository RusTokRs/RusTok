# Implementation Plan for `rustok-ui-core`

## Current state

`rustok-ui-core` owns framework-agnostic UI route context contracts, route query
intents, busy-key action tracking, and CSS accent normalization. It represents
the Headless UI foundational layer of RusToK. Currently, list views across
modules duplicate pagination and sorting math, and query parameter handling
often relies on manual string manipulation.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This crate defines headless UI data structures and contracts. It contains zero
  rendering or framework dependencies (no Leptos, no Dioxus).

## Completed foundations

1. **Standardized URL joining and query serialization.**
   Route URL manipulation uses `safe_join_url` backed by `url::Url`, preventing
   uncontrolled path and slash concatenation bugs.

2. **Canonical Headless UI pagination and sorting states.**
   `UiPaginationState` (page, per_page, total_items, offset, total_pages, has_next,
   has_previous) and `UiSortState` / `UiSortDirection` are exported, providing
   uniform pagination and sort math without framework dependencies.

3. **Framework-agnostic `UiRouteContext` and `UiRouteQueryIntent`.**
   `UiRouteContext`, `UiRouteQueryIntent`, `UiRouteQueryUpdate`, and `UiRouteQueryWrite`
   standardize route parameter update intents across Leptos, Dioxus, and headless hosts.

4. **Canonical Headless UI selection and filter state models.**
   `UiSelectionState` (toggle, select all, deselect, clear, is_all_selected,
   is_partially_selected, count) and `UiFilterRule` / `UiFilterOperator` provide
   reusable table selection and filter primitives across module list views.

## Open results

1. **Audit and standardize table state across module UI packages.**
   Done when module table/list views standardize on `UiPaginationState`, `UiSortState`,
   `UiSelectionState`, and `UiFilterRule` from `rustok-ui-core`.
   **Verification:** compilation check across module admin list views.

## Verification

- `cargo test -p rustok-ui-core --lib`
- Verification that downstream modules can compute pagination and query intents
  without framework-specific dependencies.

## Change rules

1. Never add Leptos, Dioxus, or HTML/DOM rendering dependencies to this crate.
2. Keep all contracts 100% headless and framework-agnostic.
3. Update local docs and crate README whenever new UI core contracts are added.
