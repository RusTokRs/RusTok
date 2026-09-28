# rustok-ui-grid

Enterprise-grade, framework-agnostic (FFA) DataGrid engine with column resizing, Magento-style per-column filters, and Leptos/Dioxus adapter seams for the RusToK platform.

## Architecture

Following the **Fluid Frontend Architecture (FFA)**:

- `core/`: 100% Rust, framework-agnostic grid state machine:
  - Column definitions, alignment, sizing, and pin configuration.
  - Column resizing math with min/max clamps.
  - Column filter specifications (Text, Select, NumberRange, DateRange, Boolean).
  - Sorting and multi-column sorting state.
  - Row selection and bulk actions state.
  - Dual pagination (Classic Paged vs Infinite Scroll).
- `adapters/leptos/`: Leptos 0.8 adapter rendering `<DataGrid />`:
  - Semantic `<table>` structure styled with RusToK design tokens and Tailwind CSS.
  - Resizable column headers with pointer-drag handles.
  - Interactive filter row directly beneath column headers.
  - Virtualized/paginated row rendering.
  - Infinite scroll integration using `leptos-use::use_intersection_observer`.
  - Input debouncing using `leptos-use::use_debounce_fn`.
  - Column width persistence in localStorage using `leptos-use::use_local_storage`.
- `adapters/dioxus/`: Reserved seam for Dioxus host swap.

## Usage in Modules

```rust
use rustok_ui_grid::prelude::*;

let columns = vec![
    GridColumnDef::checkbox(),
    GridColumnDef::new("type", "Type").width(120).filter(GridFilterType::select(types)),
    GridColumnDef::new("title", "Product").min_width(200).filter(GridFilterType::text()),
    GridColumnDef::new("price", "Price").width(120).filter(GridFilterType::number_range()),
    GridColumnDef::actions("actions", "Actions").width(90),
];

view! {
    <DataGrid
        columns=columns
        data=products
        key=|p| p.id.clone()
    />
}
```
