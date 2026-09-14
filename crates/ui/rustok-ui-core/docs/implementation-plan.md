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

## Open results

1. **Standardize URL joining and query serialization.**
   Done when route URL manipulation replaces string concatenation with `url::Url`,
   and query parameter parsing/encoding standardizes on `serde_qs` and
   `serde_urlencoded` (both available in workspace).
   **Depends on:** using workspace `url` and `serde_qs`.
   **Verification:** unit tests for URL construction and complex query string
   serialization/deserialization.

2. **Provide canonical Headless UI pagination and sorting states.**
   Done when `UiPaginationState` (page, per_page, total_items, total_pages,
   offset, has_next, has_previous) and `UiSortState` (field, direction) are
   exported from this crate, eliminating duplicated pagination math in
   module admin/storefront list views.
   **Depends on:** Result 1.
   **Verification:** unit tests verifying boundary conditions (page 0, empty
   dataset, single-page results).

3. **Solidify framework-agnostic `UiRouteContext` and `UiRouteQueryIntent`.**
   Done when intent writing (`push`, `replace`, `clear`) provides a uniform
   contract across Leptos router, Dioxus router, and headless external hosts.
   **Depends on:** Result 1.
   **Verification:** `cargo test -p rustok-ui-core --lib`.

## Verification

- `cargo test -p rustok-ui-core --lib`
- Verification that downstream modules can compute pagination and query intents
  without framework-specific dependencies.

## Change rules

1. Never add Leptos, Dioxus, or HTML/DOM rendering dependencies to this crate.
2. Keep all contracts 100% headless and framework-agnostic.
3. Update local docs and crate README whenever new UI core contracts are added.
