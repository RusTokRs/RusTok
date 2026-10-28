use leptos::prelude::*;

use crate::{filter_inputs::GridFilterCell, resize_handle::ColumnResizeHandle};
use rustok_grid::{
    ColumnAlign, ColumnFilters, ColumnWidths, FilterValue, GridColumnDef, SortDirection, SortState,
    has_filter_row,
};

/// Table header: one row of column titles (sort + resize) and, when at least
/// one visible column declares a filter, a Magento-style filter row.
#[component]
pub fn GridHeader(
    /// Reactive column set — visibility is toggled from the toolbar.
    columns: Signal<Vec<GridColumnDef>>,
    column_widths: Signal<ColumnWidths>,
    sort_state: Signal<SortState>,
    filters: Signal<ColumnFilters>,
    all_selected: Signal<bool>,
    /// Some — but not all — rows of the current page are selected.
    some_selected: Signal<bool>,
    on_toggle_all: Callback<bool>,
    on_sort: Callback<String>,
    on_resize: Callback<(String, u32)>,
    on_filter_change: Callback<(String, FilterValue)>,
    on_clear_filters: Callback<()>,
) -> impl IntoView {
    view! {
        <thead class="bg-muted/50 text-muted-foreground text-xs font-medium border-b border-border select-none">
            // Row 1: column titles + sort + resize handles
            <tr>
                {move || {
                    columns
                        .get()
                        .into_iter()
                        .filter(|col| col.visible)
                        .map(|col| {
                            let id = col.id.as_str().to_string();
                            let is_checkbox = col.is_checkbox();
                            let sortable = col.sortable && !is_checkbox;
                            let title = col.title.clone();
                            let width = col.width;
                            let resizable = col.resizable && width.is_flexible();

                            let current_width = {
                                let id = id.clone();
                                Signal::derive(move || {
                                    column_widths
                                        .get()
                                        .get_clamped(&id, width.current, width.min, width.max)
                                })
                            };

                            let sort_dir = {
                                let id = id.clone();
                                move || sort_state.get().is_sorted_by(&id)
                            };
                            let aria_sort = {
                                let id = id.clone();
                                move || sort_state.get().aria_sort_for(&id).to_string()
                            };

                            let align_class = match col.align {
                                ColumnAlign::Left => "text-left justify-start",
                                ColumnAlign::Center => "text-center justify-center",
                                ColumnAlign::Right => "text-right justify-end",
                            };

                            let sort_id = id.clone();
                            let sort_label = title.clone();

                            view! {
                                <th
                                    scope="col"
                                    class="relative px-3 py-2.5 font-semibold text-foreground/80 tracking-wide border-r border-border/40 last:border-r-0 group/th"
                                    style=move || {
                                        format!(
                                            "width: {}px; min-width: {}px; max-width: {}px;",
                                            current_width.get(),
                                            width.min,
                                            width.max,
                                        )
                                    }
                                    aria-sort=aria_sort
                                >
                                    <div class=format!("flex items-center gap-1.5 {align_class}")>
                                        {if is_checkbox {
                                            view! {
                                                <input
                                                    type="checkbox"
                                                    aria-label="Select all rows on this page"
                                                    prop:checked=move || all_selected.get()
                                                    prop:indeterminate=move || some_selected.get()
                                                    on:change=move |ev| on_toggle_all.run(event_target_checked(&ev))
                                                    class="rounded border-border text-primary focus:ring-primary/30 h-4 w-4 cursor-pointer"
                                                />
                                            }
                                                .into_any()
                                        } else if sortable {
                                            view! {
                                                // A real <button>: focusable, Enter/Space activated,
                                                // announced as a control by screen readers.
                                                <button
                                                    type="button"
                                                    class="cursor-pointer hover:text-foreground flex items-center gap-1 transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-primary/50 rounded"
                                                    title=format!("Sort by {sort_label}")
                                                    on:click=move |_| on_sort.run(sort_id.clone())
                                                >
                                                    <span>{title.clone()}</span>
                                                    <span
                                                        class="inline-flex text-[10px] text-muted-foreground/70"
                                                        aria-hidden="true"
                                                    >
                                                        {move || match sort_dir() {
                                                            Some(SortDirection::Asc) => "▲",
                                                            Some(SortDirection::Desc) => "▼",
                                                            None => "⇅",
                                                        }}
                                                    </span>
                                                </button>
                                            }
                                                .into_any()
                                        } else {
                                            view! {
                                                <div class="flex items-center gap-1">
                                                    <span>{title.clone()}</span>
                                                </div>
                                            }
                                                .into_any()
                                        }}
                                    </div>

                                    {if resizable {
                                        view! {
                                            <ColumnResizeHandle
                                                column_id=id
                                                column_title=title
                                                current_width=current_width
                                                min_width=width.min
                                                max_width=width.max
                                                on_resize=on_resize
                                            />
                                        }
                                            .into_any()
                                    } else {
                                        ().into_any()
                                    }}
                                </th>
                            }
                        })
                        .collect_view()
                }}
            </tr>

            // Row 2: filter row — only rendered when a visible column actually
            // provides a filter control.
            {move || {
                let cols = columns.get();
                if !has_filter_row(&cols) {
                    return ().into_any();
                }
                view! {
                    <tr class="bg-muted/30 border-t border-border/40">
                        {cols
                            .into_iter()
                            .filter(|col| col.visible)
                            .map(|col| {
                                let id = col.id.as_str().to_string();
                                let is_checkbox = col.is_checkbox();
                                let has_filter = col.has_filter();
                                let filter_type = col.filter_type.clone();
                                let filter_value = {
                                    let id = id.clone();
                                    Signal::derive(move || filters.get().get(&id).cloned())
                                };

                                view! {
                                    <td class="px-2 py-1.5 border-r border-border/40 last:border-r-0 align-middle">
                                        {if is_checkbox {
                                            view! {
                                                <button
                                                    type="button"
                                                    title="Reset all filters"
                                                    aria-label="Reset all filters"
                                                    on:click=move |_| on_clear_filters.run(())
                                                    class="text-[10px] text-muted-foreground hover:text-foreground px-1 py-0.5 rounded hover:bg-muted font-medium transition-colors"
                                                >
                                                    "✕"
                                                </button>
                                            }
                                                .into_any()
                                        } else {
                                            match filter_type.filter(|_| has_filter) {
                                                Some(filter_type) => {
                                                    view! {
                                                        <GridFilterCell
                                                            column_id=id
                                                            column_title=col.title.clone()
                                                            filter_type=filter_type
                                                            current_value=filter_value
                                                            on_change=on_filter_change
                                                        />
                                                    }
                                                        .into_any()
                                                }
                                                None => view! { <div class="h-6"></div> }.into_any(),
                                            }
                                        }}
                                    </td>
                                }
                            })
                            .collect_view()}
                    </tr>
                }
                    .into_any()
            }}
        </thead>
    }
}
