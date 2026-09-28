//! The grid itself: state ownership, wiring and layout.

use std::collections::BTreeMap;
use std::rc::Rc;

use dioxus::prelude::*;

use rustok_grid::{
    ColumnFilters, ColumnWidths, FilterValue, GridColumnDef, GridPagination, PaginationMode,
    RowSelection, SortState, visible_column_count,
};

use crate::filter_inputs::FilterCommitMode;
use crate::header::GridHeader;
use crate::pagination::GridPaginationBar;
use crate::row::GridRow;
use crate::toolbar::GridToolbar;

/// Everything a cell renderer needs to know about the cell it is rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct GridCell<T> {
    /// Payload of the row this cell belongs to.
    pub item: T,
    /// Column this cell belongs to.
    pub column_id: String,
    /// Identity of the row, as produced by `row_key`.
    pub row_id: String,
}

/// SSR-friendly data grid.
///
/// # Data flow
///
/// Every stateful slice is **uncontrolled by default and controlled on
/// demand**. For filters, sort and selection:
///
/// * nothing passed — the grid owns the state;
/// * value **and** its `on_*_change` handler passed — your value is the single
///   source of truth, the grid only reports intents;
/// * value passed without a handler — it seeds the internal state once.
///
/// Pagination has two modes, selected implicitly:
///
/// * **client-side** (no `on_page_change`): the grid owns `total` (derived from
///   `data`) and slices `data` itself;
/// * **server-side** (`on_page_change` provided): the caller owns `data` and
///   must keep `pagination.total` up to date; the grid never slices and reports
///   the *clamped, effective* page number back.
///
/// In server-side mode a filter or sort change does **not** emit a page reset:
/// you get `on_filter_change` / `on_sort_change` and decide yourself what the
/// new query looks like. Client-side, the grid jumps back to page 1 for you.
#[component]
pub fn DataGrid<T: Clone + PartialEq + 'static>(
    /// Column definitions. Visibility can then be toggled from the toolbar.
    columns: Vec<GridColumnDef>,
    /// Rows to render — the full data set (client-side) or the current page
    /// (server-side).
    data: Vec<T>,
    /// Stable, unique row identity. Used for selection and for keyed rendering.
    row_key: Callback<T, String>,
    /// Renders the content of one cell.
    cell_renderer: Callback<GridCell<T>, Element>,
    /// Shows a spinner instead of the empty-state text.
    #[props(default = false)]
    is_loading: bool,
    /// Text shown when there is nothing to render.
    #[props(default)]
    empty_message: Option<String>,
    /// Accessible name of the table, announced by screen readers.
    #[props(default)]
    aria_label: Option<String>,
    /// Emitted when a row (outside the checkbox cell) is clicked.
    #[props(default)]
    on_row_click: Option<EventHandler<T>>,
    /// Pagination state.
    #[props(default)]
    pagination: Option<GridPagination>,
    /// Emitted with the effective page number; its presence switches the grid
    /// into server-side mode.
    #[props(default)]
    on_page_change: Option<EventHandler<usize>>,
    /// Emitted with the new page size.
    #[props(default)]
    on_page_size_change: Option<EventHandler<usize>>,
    /// Emitted in infinite mode when the next chunk should be appended.
    #[props(default)]
    on_load_more: Option<EventHandler<()>>,
    /// Selectable page sizes.
    #[props(default)]
    page_size_options: Option<Vec<usize>>,
    /// Applied column filters.
    #[props(default)]
    filters: Option<ColumnFilters>,
    /// Emitted with the full filter set after every change.
    #[props(default)]
    on_filter_change: Option<EventHandler<ColumnFilters>>,
    /// When free-text filters publish their value.
    #[props(default)]
    filter_commit: FilterCommitMode,
    /// Applied sort.
    #[props(default)]
    sort_state: Option<SortState>,
    /// Emitted after every sort change.
    #[props(default)]
    on_sort_change: Option<EventHandler<SortState>>,
    /// Selected rows.
    #[props(default)]
    selection: Option<RowSelection>,
    /// Emitted after every selection change.
    #[props(default)]
    on_selection_change: Option<EventHandler<RowSelection>>,
    /// Renders actions for the current selection, e.g. "delete selected".
    #[props(default)]
    bulk_actions: Option<Callback<usize, Element>>,
) -> Element {
    // ── state ──────────────────────────────────────────────────────────────
    // Hooks first, unconditionally, and seeded from the props so that the very
    // first (server-rendered) paint is already correct.
    let initial_filters = filters.clone().unwrap_or_default();
    let initial_sort = sort_state.clone().unwrap_or_default();
    let initial_selection = selection.clone().unwrap_or_default();
    let initial_pagination = pagination.unwrap_or_default();

    let mut local_filters = use_signal(move || initial_filters);
    let mut local_sort = use_signal(move || initial_sort);
    let mut local_selection = use_signal(move || initial_selection);
    let mut local_pagination = use_signal(move || initial_pagination);
    let mut local_widths = use_signal(ColumnWidths::new);
    // Visibility overrides instead of an owned copy of `columns`: the column
    // list stays a prop, so a re-labelled or re-ordered set flows straight in.
    let mut visibility = use_signal(BTreeMap::<String, bool>::new);

    let client_side = on_page_change.is_none();
    let filters_controlled = filters.is_some() && on_filter_change.is_some();
    let sort_controlled = sort_state.is_some() && on_sort_change.is_some();
    let selection_controlled = selection.is_some() && on_selection_change.is_some();
    let pagination_controlled = pagination.is_some() && !client_side;

    let current_filters = if filters_controlled {
        filters.clone().unwrap_or_default()
    } else {
        local_filters.read().clone()
    };
    let current_sort = if sort_controlled {
        sort_state.clone().unwrap_or_default()
    } else {
        local_sort.read().clone()
    };
    let current_selection = if selection_controlled {
        selection.clone().unwrap_or_default()
    } else {
        local_selection.read().clone()
    };
    let widths = local_widths.read().clone();

    let total_rows = data.len();
    let mut current_pagination = if pagination_controlled {
        pagination.unwrap_or_default()
    } else {
        *local_pagination.read()
    };
    if client_side {
        // Deriving the total during rendering (instead of patching it from an
        // effect) keeps SSR correct — effects never run on the server.
        current_pagination.set_total(total_rows as u64);
    }

    let overrides = visibility.read().clone();
    let effective_columns = columns
        .iter()
        .cloned()
        .map(|mut column| {
            if let Some(visible) = overrides.get(column.id.as_str()) {
                column.visible = *visible;
            }
            column
        })
        .collect::<Vec<_>>();
    let colspan = visible_column_count(&effective_columns).max(1);

    let display_items: Vec<T> = if client_side {
        let (start, end) = current_pagination.slice_bounds(total_rows);
        data[start..end].to_vec()
    } else {
        data
    };
    let page_ids: Vec<String> = display_items
        .iter()
        .cloned()
        .map(|item| row_key.call(item))
        .collect();

    // "Select all" is scoped to the rows currently on screen, which is exactly
    // what the control in the header claims to do.
    let all_selected = current_selection.is_all_selected(page_ids.iter());
    let some_selected = current_selection.is_partially_selected(page_ids.iter());
    let selected_count = current_selection.count();
    let has_active_filters = !current_filters.is_empty();

    let empty_msg = empty_message.unwrap_or_else(|| "No records found.".to_string());
    let table_label = aria_label.unwrap_or_else(|| "Data grid".to_string());

    // ── intents ────────────────────────────────────────────────────────────
    // Shared snapshots: one clone per render instead of one clone per row.
    let filters_now = Rc::new(current_filters.clone());
    let sort_now = Rc::new(current_sort.clone());
    let selection_now = Rc::new(current_selection.clone());
    let columns_now = Rc::new(effective_columns.clone());

    // Applies a pagination intent and returns the state that was actually
    // adopted — never an out-of-range page a user managed to click.
    let mut apply_pagination = move |mut next: GridPagination| -> GridPagination {
        if client_side {
            next.set_total(total_rows as u64);
        }
        next.normalize();
        if !pagination_controlled {
            local_pagination.set(next);
        }
        next
    };

    // Client-side, a different result set invalidates the current page.
    let mut reset_page = move || {
        if client_side {
            let mut next = current_pagination;
            next.set_page(1);
            apply_pagination(next);
        }
    };

    let mut publish_filters = move |next: ColumnFilters| {
        if !filters_controlled {
            local_filters.set(next.clone());
        }
        reset_page();
        if let Some(handler) = on_filter_change {
            handler.call(next);
        }
    };

    let mut publish_selection = move |next: RowSelection| {
        if !selection_controlled {
            local_selection.set(next.clone());
        }
        if let Some(handler) = on_selection_change {
            handler.call(next);
        }
    };

    let filters_for_change = filters_now.clone();
    let filters_for_toolbar_clear = filters_now.clone();
    let filters_for_header_clear = filters_now;
    let sort_for_toggle = sort_now;
    let selection_for_all = selection_now.clone();
    let selection_for_row = selection_now;
    let columns_for_resize = columns_now.clone();
    let columns_for_visibility = columns_now;
    let ids_for_all = page_ids.clone();

    let rows = display_items
        .iter()
        .cloned()
        .zip(page_ids.iter().cloned())
        .map(|(item, row_id)| {
            let is_selected = current_selection.is_selected(&row_id);
            let selection_snapshot = selection_for_row.clone();
            let key = row_id.clone();
            rsx! {
                GridRow {
                    key: "{key}",
                    item: item,
                    row_id: row_id,
                    columns: effective_columns.clone(),
                    column_widths: widths.clone(),
                    is_selected: is_selected,
                    on_row_click: on_row_click,
                    cell_renderer: cell_renderer,
                    on_toggle_select: move |id: String| {
                        let mut next = (*selection_snapshot).clone();
                        next.toggle(id);
                        publish_selection(next);
                    },
                }
            }
        });

    let empty_state: Option<Element> = display_items.is_empty().then(|| {
        rsx! {
            tr {
                td {
                    colspan: "{colspan}",
                    class: "py-12 text-center text-sm text-muted-foreground",
                    "aria-live": "polite",
                    if is_loading {
                        div { class: "inline-flex items-center gap-2",
                            div {
                                class: "h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent",
                                "aria-hidden": "true",
                            }
                            span { "Loading data..." }
                        }
                    } else {
                        p { "{empty_msg}" }
                    }
                }
            }
        }
    });

    rsx! {
        div { class: "w-full rounded-xl border border-border/80 bg-card shadow-sm flex flex-col overflow-hidden",

            GridToolbar {
                total_count: current_pagination.total,
                selected_count: selected_count,
                has_active_filters: has_active_filters,
                columns: effective_columns.clone(),
                bulk_actions: bulk_actions,
                on_clear_filters: move |_| {
                    let mut next = (*filters_for_toolbar_clear).clone();
                    if next.clear() {
                        publish_filters(next);
                    }
                },
                on_toggle_column_visibility: move |id: String| {
                    let current = columns_for_visibility
                        .iter()
                        .find(|column| column.id.as_str() == id)
                        .map(|column| column.visible)
                        .unwrap_or(true);
                    visibility.write().insert(id, !current);
                },
            }

            div { class: "w-full overflow-x-auto relative",
                table { class: "w-full border-collapse text-left", "aria-label": "{table_label}",

                    GridHeader {
                        columns: effective_columns.clone(),
                        column_widths: widths.clone(),
                        sort_state: current_sort.clone(),
                        filters: current_filters.clone(),
                        all_selected: all_selected,
                        some_selected: some_selected,
                        filter_commit: filter_commit,
                        on_toggle_all: move |check_all: bool| {
                            let mut next = (*selection_for_all).clone();
                            if check_all {
                                next.select_all(ids_for_all.clone());
                            } else {
                                next.deselect_all(ids_for_all.iter());
                            }
                            publish_selection(next);
                        },
                        on_sort: move |id: String| {
                            let mut next = (*sort_for_toggle).clone();
                            next.toggle(&id);
                            if !sort_controlled {
                                local_sort.set(next.clone());
                            }
                            reset_page();
                            if let Some(handler) = on_sort_change {
                                handler.call(next);
                            }
                        },
                        on_resize: move |(id, width): (String, u32)| {
                            let bounds = columns_for_resize
                                .iter()
                                .find(|column| column.id.as_str() == id)
                                .map(|column| (column.width.min, column.width.max));
                            if let Some((min, max)) = bounds {
                                local_widths.write().set_clamped(id, width, min, max);
                            }
                        },
                        on_filter_change: move |(id, value): (String, FilterValue)| {
                            let mut next = (*filters_for_change).clone();
                            if next.set(id, value) {
                                publish_filters(next);
                            }
                        },
                        on_clear_filters: move |_| {
                            let mut next = (*filters_for_header_clear).clone();
                            if next.clear() {
                                publish_filters(next);
                            }
                        },
                    }

                    tbody { class: "divide-y divide-border/40 bg-background",
                        {rows}
                        {empty_state}
                    }
                }
            }

            GridPaginationBar {
                pagination: current_pagination,
                page_size_options: page_size_options,
                on_page_change: move |requested: usize| {
                    let mut next = current_pagination;
                    next.set_page(requested);
                    let applied = apply_pagination(next);
                    if let Some(handler) = on_page_change {
                        handler.call(applied.page);
                    }
                },
                on_page_size_change: move |size: usize| {
                    let mut next = current_pagination;
                    next.set_page_size(size);
                    let applied = apply_pagination(next);
                    if let Some(handler) = on_page_size_change {
                        handler.call(applied.page_size);
                    }
                    // A new page size sends the user back to page 1; a
                    // server-side consumer has to learn about that too — but
                    // only when it actually happened.
                    if applied.page != current_pagination.page {
                        if let Some(handler) = on_page_change {
                            handler.call(applied.page);
                        }
                    }
                },
                on_mode_change: move |mode: PaginationMode| {
                    let mut next = current_pagination;
                    next.set_mode(mode);
                    apply_pagination(next);
                },
                on_load_more: move |_| {
                    if !current_pagination.has_next {
                        return;
                    }
                    let mut next = current_pagination;
                    next.next_page();
                    let applied = apply_pagination(next);
                    if let Some(handler) = on_load_more {
                        handler.call(());
                    } else if let Some(handler) = on_page_change {
                        handler.call(applied.page);
                    }
                },
            }
        }
    }
}
