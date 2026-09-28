use leptos::prelude::*;

use crate::{
    header::GridHeader,
    pagination::GridPaginationBar,
    row::GridRow,
    toolbar::GridToolbar,
};
use rustok_grid::{
    ColumnFilters, ColumnWidths, FilterValue, GridColumnDef, GridPagination, PaginationMode,
    RowSelection, SortState,
};

#[component]
pub fn DataGrid<T, K, KF>(
    columns: Vec<GridColumnDef>,
    data: Signal<Vec<T>>,
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

    let empty_msg = empty_message.unwrap_or_else(|| "No records found.".to_string());
    let loading_signal = is_loading.unwrap_or_else(|| Signal::derive(|| false));

    // Handle column resize
    let handle_resize = Callback::new(move |(col_id, width): (String, u32)| {
        local_widths.update(|w| w.set(col_id, width));
    });

    // Handle single filter change
    let handle_filter_change = Callback::new(move |(col_id, val): (String, FilterValue)| {
        local_filters.update(|f| f.set(col_id, val));
        local_pagination.update(|p| p.set_page(1));
        if let Some(on_change) = on_filter_change {
            on_change.run(local_filters.get());
        }
    });

    // Handle clear all filters
    let handle_clear_filters = Callback::new(move |_| {
        local_filters.update(|f| f.clear());
        local_pagination.update(|p| p.set_page(1));
        if let Some(on_change) = on_filter_change {
            on_change.run(local_filters.get());
        }
    });

    // Handle column sort
    let handle_sort = Callback::new(move |col_id: String| {
        local_sort.update(|s| s.toggle(&col_id));
        if let Some(on_sort) = on_sort_change {
            on_sort.run(local_sort.get());
        }
    });

    // Handle toggle column visibility
    let handle_toggle_visibility = Callback::new(move |col_id: String| {
        local_columns.update(|cols| {
            for c in cols.iter_mut() {
                if c.id.0 == col_id {
                    c.visible = !c.visible;
                }
            }
        });
    });

    // Handle row selection
    let handle_toggle_row_select = Callback::new(move |id: String| {
        local_selection.update(|s| s.toggle(id));
        if let Some(on_sel) = on_selection_change {
            on_sel.run(local_selection.get());
        }
    });

    // Handle toggle all current page selection
    let handle_toggle_all_select = Callback::new(move |check_all: bool| {
        let current_ids: Vec<String> = data.get().iter().map(|item| key_fn(item).to_string()).collect();
        local_selection.update(|s| {
            if check_all {
                s.select_all(current_ids);
            } else {
                s.deselect_all(current_ids.iter().map(|s| s.as_str()));
            }
        });
        if let Some(on_sel) = on_selection_change {
            on_sel.run(local_selection.get());
        }
    });

    // Are all current rows selected?
    let all_selected = Signal::derive(move || {
        let items = data.get();
        if items.is_empty() {
            return false;
        }
        let sel = local_selection.get();
        items.iter().all(|item| sel.is_selected(&key_fn(item).to_string()))
    });

    // Has selectable column?
    let has_selectable = Signal::derive(move || {
        local_columns.get().iter().any(|c| c.id.0 == "__checkbox")
    });

    // Page change handler
    let handle_page_change = Callback::new(move |new_page: usize| {
        local_pagination.update(|p| p.set_page(new_page));
        if let Some(cb) = on_page_change {
            cb.run(new_page);
        }
    });

    // Page size change handler
    let handle_page_size_change = Callback::new(move |new_size: usize| {
        local_pagination.update(|p| p.set_page_size(new_size));
        if let Some(cb) = on_page_size_change {
            cb.run(new_size);
        }
    });

    // Mode change handler (Paged vs Infinite)
    let handle_mode_change = Callback::new(move |mode: PaginationMode| {
        local_pagination.update(|p| p.mode = mode);
    });

    // Load more handler (infinite scroll)
    let handle_load_more = Callback::new(move |_| {
        if let Some(cb) = on_load_more {
            cb.run(());
        } else {
            let next_page = local_pagination.get().page + 1;
            local_pagination.update(|p| p.page = next_page);
            if let Some(cb) = on_page_change {
                cb.run(next_page);
            }
        }
    });

    let total_count = Signal::derive(move || local_pagination.get().total);
    let selected_count = Signal::derive(move || local_selection.get().count());
    let has_active_filters = Signal::derive(move || !local_filters.get().is_empty());

    view! {
        <div class="w-full rounded-xl border border-border/80 bg-card shadow-sm flex flex-col overflow-hidden">
            // Toolbar (selection counter, columns config, filters status)
            <GridToolbar
                total_count=total_count
                selected_count=selected_count
                has_active_filters=has_active_filters
                on_clear_filters=handle_clear_filters
                columns=local_columns.into()
                on_toggle_column_visibility=handle_toggle_visibility
                bulk_actions=bulk_actions
            />

            // Table Scroll Container
            <div class="w-full overflow-x-auto relative">
                <table class="w-full border-collapse text-left">
                    <GridHeader
                        columns=local_columns.get()
                        column_widths=local_widths.into()
                        sort_state=local_sort.into()
                        filters=local_filters.into()
                        all_selected=all_selected
                        has_selectable=has_selectable.get()
                        on_toggle_all=handle_toggle_all_select
                        on_sort=handle_sort
                        on_resize=handle_resize
                        on_filter_change=handle_filter_change
                        on_clear_filters=handle_clear_filters
                    />

                    <tbody class="divide-y divide-border/40 bg-background">
                        {move || {
                            let items = data.get();
                            let loading = loading_signal.get();

                            if loading && items.is_empty() {
                                view! {
                                    <tr>
                                        <td
                                            colspan=local_columns.get().len()
                                            class="py-12 text-center text-sm text-muted-foreground"
                                        >
                                            <div class="inline-flex items-center gap-2">
                                                <div class="h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent" />
                                                <span>"Loading data..."</span>
                                            </div>
                                        </td>
                                    </tr>
                                }
                                .into_any()
                            } else if items.is_empty() {
                                let msg = empty_msg.clone();
                                view! {
                                    <tr>
                                        <td
                                            colspan=local_columns.get().len()
                                            class="py-12 text-center text-sm text-muted-foreground"
                                        >
                                            <p>{msg}</p>
                                        </td>
                                    </tr>
                                }
                                .into_any()
                            } else {
                                items
                                    .into_iter()
                                    .map(|item| {
                                        let row_id = key_fn(&item).to_string();
                                        let is_sel = {
                                            let id = row_id.clone();
                                            Signal::derive(move || local_selection.get().is_selected(&id))
                                        };

                                        view! {
                                            <GridRow
                                                item=item
                                                row_id=row_id
                                                columns=local_columns.get()
                                                column_widths=local_widths.into()
                                                is_selected=is_sel
                                                on_toggle_select=handle_toggle_row_select
                                                on_row_click=on_row_click
                                                cell_renderer=cell_renderer
                                            />
                                        }
                                    })
                                    .collect_view()
                                    .into_any()
                            }
                        }}
                    </tbody>
                </table>
            </div>

            // Pagination & Infinite Scroll Footer
            <GridPaginationBar
                pagination=local_pagination.into()
                on_page_change=handle_page_change
                on_page_size_change=handle_page_size_change
                on_mode_change=handle_mode_change
                on_load_more=handle_load_more
            />
        </div>
    }
}
