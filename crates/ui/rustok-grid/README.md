# rustok-grid

Framework-agnostic DataGrid state machine, column math, and filtering engine for the RusToK platform.

## Architecture

Following Hexagonal / Ports & Adapters architecture, `rustok-grid` is 100% pure Rust with zero web framework dependencies:

- **Columns**: column definitions, alignment, sizing, and pin configuration.
- **Resize**: column resizing math with min/max clamps.
- **Filters**: column filter specifications (Text, Select, NumberRange, DateRange, Boolean).
- **Sorting**: deterministic single-column sort state machine (the adapter can delegate actual ordering to the server).
- **Selection**: row selection and bulk action tracking.
- **Pagination**: dual pagination models (classic paged vs infinite scroll), with overflow-safe bounds and one-based display indexes.
- **State**: unified grid state composite.

## Adapters

- [`rustok-grid-leptos`](./leptos): SSR-first Leptos 0.8 `<DataGrid />` UI adapter.
