# rustok-grid-dioxus

Dioxus 0.6 DataGrid adapter for RusToK.

The crate is a thin **adapter** over [`rustok-grid`](../): all state and all the
maths (widths, clamping, pagination, filter normalization, selection) live in
the framework-agnostic core, here we only turn them into markup and DOM events.
It is the exact functional twin of `rustok-grid-leptos`.

```toml
[dependencies]
rustok-grid-dioxus = { path = "crates/ui/rustok-grid/dioxus" }
```

## Components

| Component            | Role                                                                   |
| -------------------- | ---------------------------------------------------------------------- |
| `DataGrid`           | owns the state, wires everything together, slices client-side pages     |
| `GridToolbar`        | selection banner + bulk actions, filter reset, column visibility picker |
| `GridHeader`         | column row (sorting, resizing, select-all) + filter row                 |
| `GridRow`            | one `<tr>`, delegates cell content to `cell_renderer`                   |
| `GridFilterCell`     | text / select / number range / date range / boolean filter controls     |
| `ColumnResizeHandle` | pointer + keyboard column resizing (WAI-ARIA window splitter)           |
| `GridPaginationBar`  | range summary, page size, mode switch, pager, infinite-scroll sentinel  |

## Usage

```rust
use dioxus::prelude::*;
use rustok_grid_dioxus::prelude::*;

#[derive(Clone, PartialEq)]
struct Product {
    id: String,
    title: String,
    price: f64,
}

#[component]
fn ProductGrid(products: Vec<Product>) -> Element {
    let columns = vec![
        GridColumnDef::checkbox(),
        GridColumnDef::new("title", "Title").filter(GridFilterType::text()),
        GridColumnDef::new("price", "Price")
            .align(ColumnAlign::Right)
            .filter(GridFilterType::number_range()),
    ];

    rsx! {
        DataGrid {
            columns,
            data: products,
            aria_label: Some("Products".to_string()),
            row_key: Callback::new(|product: Product| product.id.clone()),
            cell_renderer: Callback::new(|cell: GridCell<Product>| {
                match cell.column_id.as_str() {
                    "title" => rsx! { span { class: "font-medium", "{cell.item.title}" } },
                    "price" => rsx! { span { "{cell.item.price:.2}" } },
                    _ => rsx! {},
                }
            }),
            on_selection_change: move |selection: RowSelection| {
                tracing::debug!(count = selection.count(), "selection changed");
            },
        }
    }
}
```

## Data flow

Every stateful slice — filters, sort, selection, pagination — is **uncontrolled
by default and controlled on demand**:

- pass nothing → the grid owns the state internally;
- pass a value **and** its `on_*_change` handler → your value is the single
  source of truth, the grid only reports intents;
- pass a value **without** a handler → it seeds the internal state once.

Pagination picks its mode implicitly:

| Mode            | Trigger              | Who owns `total`     | Who slices `data` |
| --------------- | -------------------- | -------------------- | ----------------- |
| **client-side** | no `on_page_change`  | the grid (`data.len()`) | the grid       |
| **server-side** | `on_page_change` set | you (`pagination.total`) | you           |

In server-side mode a filter/sort change does *not* emit a page reset: you get
`on_filter_change` / `on_sort_change` and decide what the new query looks like.
Client-side, the grid jumps back to page 1 for you.

## Deliberate differences from the Leptos adapter

Same components, same props, same markup and the same Tailwind tokens — with
three renderer-driven deviations:

1. **No debounce on text filters.** Dioxus ships no cross-platform timer, and
   this adapter refuses to pull in `gloo-timers`/`futures-timer` just for that.
   The trade-off is explicit instead: `filter_commit` is `FilterCommitMode::OnCommit`
   by default (publish on `Enter`, blur or a native `change`, `Escape` reverts)
   and can be switched to `FilterCommitMode::Live` for client-side data sets.
2. **Resizing uses a drag overlay, not pointer capture.** While dragging, a
   transparent full-viewport `div` receives `pointermove`/`pointerup`, which
   survives fast cursor movement without touching `web-sys` — so it keeps
   working in the desktop and native renderers. Keyboard resizing (arrows,
   `Home`/`End`) is identical to the Leptos adapter.
3. **Select-all is a `role="checkbox"` button.** `<input type="checkbox">`
   cannot express `indeterminate` declaratively in rsx, so the tri-state is
   rendered as a button with `aria-checked="true" | "false" | "mixed"`.

Infinite scrolling uses the `onvisible` special attribute (Dioxus' built-in
intersection observer — no JavaScript, no `web-sys`), plus a real **Load more**
button, because a scroll-only trigger is unreachable by keyboard.

## Accessibility

- `<th scope="col">` with `aria-sort` on every sortable column, sorting through
  a real `<button>`.
- Resize handles follow the WAI-ARIA *window splitter* pattern:
  `role="separator"`, `aria-orientation`, `aria-valuemin/max/now`, `tabindex=0`,
  arrow keys and `Home`/`End`.
- Tri-state select-all, `aria-live` on the empty state, the range summary and
  the selection banner, accessible names on every filter control.
- The column picker closes on outside click and on `Escape`.

## Rendering

The component tree is pure: no state is created inside an effect and nothing is
written to a signal during rendering, so `dioxus-ssr` output and the hydrated
client tree are identical — the first paint is already the final paint.

## Build

The crate is intentionally **not** a workspace member (same as
`rustok-forms-dioxus`): the workspace lockfile is Leptos-only and CI builds with
`--locked`. Build it from its own directory, or add
`crates/ui/rustok-grid/dioxus` to `workspace.members` and regenerate
`Cargo.lock` once Dioxus is meant to ship.

## License

Same as the workspace.
