use leptos::prelude::*;

use crate::{filter_inputs::GridFilterCell, resize_handle::ColumnResizeHandle};
use rustok_grid::{
    ColumnAlign, ColumnFilters, ColumnWidths, FilterValue, GridColumnDef,
    SortDirection, SortState,
};

#[component]
pub fn GridHeader(
    columns: Vec<GridColumnDef>,
    column_widths: Signal<ColumnWidths>,
    sort_state: Signal<SortState>,
    filters: Signal<ColumnFilters>,
    all_selected: Signal<bool>,
    has_selectable: bool,
    on_toggle_all: Callback<bool>,
    on_sort: Callback<String>,
    on_resize: Callback<(String, u32)>,
    on_filter_change: Callback<(String, FilterValue)>,
    on_clear_filters: Callback<()>,
) -> impl IntoView {
    let _ = has_selectable;
    let has_any_filterable = columns.iter().any(|c| c.filterable);
    let columns_for_filters = columns.clone();

    view! {
        <thead class="bg-muted/50 text-muted-foreground text-xs font-medium border-b border-border select-none">
            // Row 1: Column Titles + Sort + Resize Handles
            <tr>
                {columns
                    .into_iter()
                    .filter(|col| col.visible)
                    .map(|col| {
                        let id = col.id.0.clone();
                        let col_id_for_sort = id.clone();
                        let col_id_for_sort_dir = id.clone();
                        let col_id_for_resize = id.clone();
                        let is_checkbox = id == "__checkbox";
                        let id_for_w = id.clone();
                        let default_w = col.width.current;
                        let current_w = Signal::derive(move || {
                            column_widths.get().get_clamped(&id_for_w, default_w, col.width.min, col.width.max)
                        });

                        let sort_dir = move || {
                            sort_state.get().is_sorted_by(&col_id_for_sort_dir)
                        };

                        let align_class = match col.align {
                            ColumnAlign::Left => "text-left justify-start",
                            ColumnAlign::Center => "text-center justify-center",
                            ColumnAlign::Right => "text-right justify-end",
                        };

                        view! {
                            <th
                                class="relative px-3 py-2.5 font-semibold text-foreground/80 tracking-wide border-r border-border/40 last:border-r-0 group/th"
                                style=move || format!("width: {}px; min-width: {}px; max-width: {}px;", current_w.get(), col.width.min, col.width.max)
                            >
                                <div class=format!("flex items-center gap-1.5 {align_class}")>
                                    {if is_checkbox {
                                        view! {
                                            <input
                                                type="checkbox"
                                                prop:checked=move || all_selected.get()
                                                on:change=move |ev| on_toggle_all.run(event_target_checked(&ev))
                                                class="rounded border-border text-primary focus:ring-primary/30 h-4 w-4 cursor-pointer"
                                            />
                                        }
                                        .into_any()
                                    } else {
                                        let col_title = col.title.clone();
                                        let sortable = col.sortable;
                                        view! {
                                            <div
                                                class=if sortable {
                                                    "cursor-pointer hover:text-foreground flex items-center gap-1 transition-colors"
                                                } else {
                                                    "flex items-center gap-1"
                                                }
                                                on:click=move |_| {
                                                    if sortable {
                                                        on_sort.run(col_id_for_sort.clone());
                                                    }
                                                }
                                            >
                                                <span>{col_title}</span>
                                                {if sortable {
                                                    view! {
                                                        <span class="inline-flex text-[10px] text-muted-foreground/70">
                                                            {move || match sort_dir() {
                                                                Some(SortDirection::Asc) => "▲",
                                                                Some(SortDirection::Desc) => "▼",
                                                                None => "⇅",
                                                            }}
                                                        </span>
                                                    }
                                                    .into_any()
                                                } else {
                                                    ().into_any()
                                                }}
                                            </div>
                                        }
                                        .into_any()
                                    }}
                                </div>

                                {if col.resizable {
                                    view! {
                                        <ColumnResizeHandle
                                            column_id=col_id_for_resize
                                            current_width=current_w
                                            min_width=col.width.min
                                            max_width=col.width.max
                                            on_resize=on_resize
                                        />
                                    }
                                    .into_any()
                                } else {
                                    ().into_any()
                                }}
                            </th>
                        }
                        .into_any()
                    })
                    .collect_view()}
            </tr>

            // Row 2: Magento-style Filter Row (under columns)
            {if has_any_filterable {
                view! {
                    <tr class="bg-muted/30 border-t border-border/40">
                        {columns_for_filters
                            .into_iter()
                            .filter(|col| col.visible)
                            .map(|col| {
                                let id = col.id.0.clone();
                                let is_checkbox = id == "__checkbox";
                                let filter_type = col.filter_type.clone();

                                let filter_val = {
                                    let col_id = id.clone();
                                    Signal::derive(move || filters.get().get(&col_id).cloned())
                                };

                                view! {
                                    <td class="px-2 py-1.5 border-r border-border/40 last:border-r-0 align-middle">
                                        {if is_checkbox {
                                            view! {
                                                <button
                                                    type="button"
                                                    title="Reset all filters"
                                                    on:click=move |_| on_clear_filters.run(())
                                                    class="text-[10px] text-muted-foreground hover:text-foreground px-1 py-0.5 rounded hover:bg-muted font-medium transition-colors"
                                                >
                                                    "✕"
                                                </button>
                                            }
                                            .into_any()
                                        } else if let Some(ft) = filter_type {
                                            view! {
                                                <GridFilterCell
                                                    column_id=id
                                                    filter_type=ft
                                                    current_value=filter_val
                                                    on_change=on_filter_change
                                                />
                                            }
                                            .into_any()
                                        } else {
                                            view! { <div class="h-6"></div> }.into_any()
                                        }}
                                    </td>
                                }
                                .into_any()
                            })
                            .collect_view()}
                    </tr>
                }
                .into_any()
            } else {
                ().into_any()
            }}
        </thead>
    }
}
