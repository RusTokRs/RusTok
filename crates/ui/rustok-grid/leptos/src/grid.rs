use leptos::prelude::*;

use crate::{
    header::GridHeader, pagination::GridPaginationBar, row::GridRow, toolbar::GridToolbar,
};
use rustok_grid::{
    ColumnFilters, ColumnWidths, FilterValue, GridColumnDef, GridPagination, PaginationMode,
    RowSelection, SortState, visible_column_count,
};

/// SSR-first data grid.
///
/// # Data flow
///
/// The component is **uncontrolled by default and controlled on demand**:
/// every piece of state (filters, sort, selection, pagination) can be handed
/// in as an `RwSignal` and is otherwise created locally.
///
/// Pagination has two modes, selected implicitly:
///
/// * **client-side** (no `on_page_change`): the grid owns `total` (derived
///   from `data`) and slices `data` itself.
/// * **server-side** (`on_page_change` provided): the caller owns `data` and
///   must keep `pagination.total` up to date; the grid never slices and
///   reports the *clamped, effective* page number back.
#[component]
pub fn DataGrid<T, K, KF>(
    /// Column definitions. Visibility is then owned by the grid (toolbar).
    columns: Vec<GridColumnDef>,
    data: Signal<Vec<T>>,
    /// Stable, unique row identity. Used for selection and for keyed
    /// rendering, so rows are not rebuilt on every data change.
    key_fn: KF,
    cell_renderer: Callback<(T, String), AnyView>,
    #[prop(optional)] is_loading: Option<Signal<bool>>,
    #[prop(optional)] empty_message: Option<String>,
    #[prop(optional)] on_row_click: Option<Callback<T>>,
    #[prop(optional)] pagination: Option<RwSignal<GridPagination>>,
    #[prop(optional)] on_page_change: Option<Callback<usize>>,
    #[prop(optional)] on_page_size_change: Option<Callback<usize>>,
    #[prop(optional)] on_load_more: Option<Callback<()>>,
    #[prop(optional)] filters: Option<RwSignal<ColumnFilters>>,
    #[prop(optional)] on_filter_change: Option<Callback<ColumnFilters>>,
    #[prop(optional)] sort_state: Option<RwSignal<SortState>>,
    #[prop(optional)] on_sort_change: Option<Callback<SortState>>,
    #[prop(optional)] selection: Option<RwSignal<RowSelection>>,
    #[prop(optional)] on_selection_change: Option<Callback<RowSelection>>,
    #[prop(optional)] bulk_actions: Option<Callback<usize, AnyView>>,
    /// Accessible name of the table, announced by screen readers.
    #[prop(optional)]
    aria_label: Option<String>,
) -> impl IntoView
where
    T: Send + Sync + Clone + 'static,
    K: std::fmt::Display + 'static,
    KF: Fn(&T) -> K + Send + Sync + Copy + 'static,
{
    let local_columns = RwSignal::new(columns);
    let local_widths = RwSignal::new(ColumnWidths::new());
    let local_filters = filters.unwrap_or_else(|| RwSignal::new(ColumnFilters::new()));
    let local_sort = sort_state.unwrap_or_else(|| RwSignal::new(SortState::default()));
    let local_selection = selection.unwrap_or_else(|| RwSignal::new(RowSelection::new()));
    let local_pagination = pagination.unwrap_or_else(|| RwSignal::new(GridPagination::default()));

    let empty_msg =
        StoredValue::new(empty_message.unwrap_or_else(|| "No records found.".to_string()));
    let table_label = aria_label.unwrap_or_else(|| "Data grid".to_string());
    let loading_signal = is_loading.unwrap_or_else(|| Signal::derive(|| false));

    // Without `on_page_change` the caller hands us the full data set, so the
    // grid owns totals and slicing.
    let client_side = on_page_change.is_none();

    // Pagination as it should be rendered. Deriving `total` (instead of only
    // patching it from an `Effect`) keeps the first SSR paint correct —
    // effects never run on the server.
    let effective_pagination = Signal::derive(move || {
        let mut pagination = local_pagination.get();
        if client_side {
            pagination.set_total(data.get().len() as u64);
        }
        pagination
    });

    // Keep the caller-visible signal in sync on the client.
    if client_side {
        Effect::new(move |_| {
            let total = data.get().len() as u64;
            if local_pagination.get_untracked().total != total {
                local_pagination.update(|pagination| pagination.set_total(total));
            }
        });
    }

    // Rows to render: the current page (client-side) or whatever the caller
    // provided (server-side).
    let display_items = move || {
        let items = data.get();
        if !client_side {
            return items;
        }
        let (start, end) = effective_pagination.get().slice_bounds(items.len());
        items[start..end].to_vec()
    };

    let visible_colspan = move || visible_column_count(&local_columns.get()).max(1);

    let notify_selection = move || {
        if let Some(on_sel) = on_selection_change {
            on_sel.run(local_selection.get_untracked());
        }
    };

    let notify_filters = move || {
        if let Some(on_change) = on_filter_change {
            on_change.run(local_filters.get_untracked());
        }
    };

    let handle_resize = Callback::new(move |(col_id, width): (String, u32)| {
        let bounds = local_columns
            .get_untracked()
            .iter()
            .find(|c| c.id.as_str() == col_id)
            .map(|c| (c.width.min, c.width.max));
        if let Some((min, max)) = bounds {
            local_widths.update(|widths| {
                widths.set_clamped(col_id, width, min, max);
            });
        }
    });

    let handle_filter_change = Callback::new(move |(col_id, value): (String, FilterValue)| {
        let mut changed = false;
        local_filters.update(|filters| {
            changed = filters.set(col_id, value);
        });
        if changed {
            // A different result set invalidates the current page.
            local_pagination.update(|pagination| pagination.set_page(1));
            notify_filters();
        }
    });

    let handle_clear_filters = Callback::new(move |_| {
        let mut changed = false;
        local_filters.update(|filters| {
            changed = filters.clear();
        });
        if changed {
            local_pagination.update(|pagination| pagination.set_page(1));
            notify_filters();
        }
    });

    let handle_sort = Callback::new(move |col_id: String| {
        local_sort.update(|sort| sort.toggle(&col_id));
        local_pagination.update(|pagination| pagination.set_page(1));
        if let Some(on_sort) = on_sort_change {
            on_sort.run(local_sort.get_untracked());
        }
    });

    let handle_toggle_visibility = Callback::new(move |col_id: String| {
        local_columns.update(|cols| {
            for column in cols.iter_mut() {
                if column.id.as_str() == col_id {
                    column.visible = !column.visible;
                }
            }
        });
    });

    let handle_toggle_row_select = Callback::new(move |id: String| {
        local_selection.update(|selection| {
            selection.toggle(id);
        });
        notify_selection();
    });

    // "Select all" is scoped to the rows currently on screen, which is what
    // the checkbox in the header claims to do.
    let page_row_ids = move || {
        display_items()
            .iter()
            .map(|item| key_fn(item).to_string())
            .collect::<Vec<_>>()
    };

    let handle_toggle_all_select = Callback::new(move |check_all: bool| {
        let current_ids = page_row_ids();
        local_selection.update(|selection| {
            if check_all {
                selection.select_all(current_ids.clone());
            } else {
                selection.deselect_all(current_ids.iter().map(String::as_str));
            }
        });
        notify_selection();
    });

    let all_selected =
        Signal::derive(move || local_selection.get().is_all_selected(page_row_ids()));
    let some_selected =
        Signal::derive(move || local_selection.get().is_partially_selected(page_row_ids()));

    let handle_page_change = Callback::new(move |requested: usize| {
        let mut effective = requested;
        local_pagination.update(|pagination| {
            effective = pagination.go_to_page(requested);
        });
        if let Some(cb) = on_page_change {
            // Report the page the grid actually switched to, never an
            // out-of-range number a user managed to click.
            cb.run(effective);
        }
    });

    let handle_page_size_change = Callback::new(move |new_size: usize| {
        let previous_page = local_pagination.get_untracked().page;
        local_pagination.update(|pagination| pagination.set_page_size(new_size));
        let current = local_pagination.get_untracked();
        if let Some(cb) = on_page_size_change {
            cb.run(current.page_size);
        }
        // Changing the page size sends the user back to page 1; a server-side
        // consumer has to learn about that too — but only when it happened.
        if previous_page != current.page
            && let Some(cb) = on_page_change
        {
            cb.run(current.page);
        }
    });

    let handle_mode_change = Callback::new(move |mode: PaginationMode| {
        local_pagination.update(|pagination| pagination.set_mode(mode));
    });

    let handle_load_more = Callback::new(move |_| {
        if !effective_pagination.get_untracked().has_next {
            return;
        }
        let mut next = effective_pagination.get_untracked().page.saturating_add(1);
        local_pagination.update(|pagination| {
            next = pagination.go_to_page(next);
        });
        if let Some(cb) = on_load_more {
            cb.run(());
        } else if let Some(cb) = on_page_change {
            cb.run(next);
        }
    });

    let total_count = Signal::derive(move || effective_pagination.get().total);
    let selected_count = Signal::derive(move || local_selection.get().count());
    let has_active_filters = Signal::derive(move || !local_filters.get().is_empty());

    view! {
        <div class="w-full rounded-xl border border-border/80 bg-card shadow-sm flex flex-col overflow-hidden">
            <GridToolbar
                total_count=total_count
                selected_count=selected_count
                has_active_filters=has_active_filters
                on_clear_filters=handle_clear_filters
                columns=local_columns.into()
                on_toggle_column_visibility=handle_toggle_visibility
                bulk_actions=bulk_actions
            />

            <div class="w-full overflow-x-auto relative">
                <table class="w-full border-collapse text-left" aria-label=table_label>
                    <GridHeader
                        columns=local_columns.into()
                        column_widths=local_widths.into()
                        sort_state=local_sort.into()
                        filters=local_filters.into()
                        all_selected=all_selected
                        some_selected=some_selected
                        on_toggle_all=handle_toggle_all_select
                        on_sort=handle_sort
                        on_resize=handle_resize
                        on_filter_change=handle_filter_change
                        on_clear_filters=handle_clear_filters
                    />

                    <tbody class="divide-y divide-border/40 bg-background">
                        <For
                            each=move || display_items()
                            key=move |item: &T| key_fn(item).to_string()
                            children=move |item: T| {
                                let row_id = key_fn(&item).to_string();
                                let is_selected = {
                                    let id = row_id.clone();
                                    Signal::derive(move || local_selection.get().is_selected(&id))
                                };
                                view! {
                                    <GridRow
                                        item=item
                                        row_id=row_id
                                        columns=local_columns.into()
                                        column_widths=local_widths.into()
                                        is_selected=is_selected
                                        on_toggle_select=handle_toggle_row_select
                                        on_row_click=on_row_click
                                        cell_renderer=cell_renderer
                                    />
                                }
                            }
                        />

                        {move || {
                            if !display_items().is_empty() {
                                return ().into_any();
                            }
                            let loading = loading_signal.get();
                            view! {
                                <tr>
                                    <td
                                        colspan=visible_colspan
                                        class="py-12 text-center text-sm text-muted-foreground"
                                        aria-live="polite"
                                    >
                                        {if loading {
                                            view! {
                                                <div class="inline-flex items-center gap-2">
                                                    <div
                                                        class="h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent"
                                                        aria-hidden="true"
                                                    />
                                                    <span>"Loading data..."</span>
                                                </div>
                                            }
                                                .into_any()
                                        } else {
                                            view! { <p>{empty_msg.get_value()}</p> }.into_any()
                                        }}
                                    </td>
                                </tr>
                            }
                                .into_any()
                        }}
                    </tbody>
                </table>
            </div>

            <GridPaginationBar
                pagination=effective_pagination
                on_page_change=handle_page_change
                on_page_size_change=handle_page_size_change
                on_mode_change=handle_mode_change
                on_load_more=handle_load_more
            />
        </div>
    }
}
