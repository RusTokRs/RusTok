//! Table head: the column row and the optional filter row.

use dioxus::prelude::*;

use rustok_grid::{
    ColumnAlign, ColumnFilters, ColumnWidths, FilterValue, GridColumnDef, SortDirection, SortState,
    has_filter_row, visible_columns,
};

use crate::filter_inputs::{FilterCommitMode, GridFilterCell};
use crate::resize_handle::ColumnResizeHandle;

/// Table head with the column row and, when at least one visible column
/// declares a filter, a second row of filter controls.
#[component]
pub fn GridHeader(
    /// Columns to render, in order. Hidden ones are skipped.
    columns: Vec<GridColumnDef>,
    /// User-applied column widths.
    column_widths: ColumnWidths,
    /// Current sort, used for `aria-sort` and the direction glyph.
    sort_state: SortState,
    /// Currently applied filters.
    filters: ColumnFilters,
    /// Every row on the current page is selected.
    all_selected: bool,
    /// Some — but not all — rows on the current page are selected.
    some_selected: bool,
    /// When free-text filters publish their value.
    #[props(default)]
    filter_commit: FilterCommitMode,
    /// Emitted with the desired state of the "select all" control.
    on_toggle_all: EventHandler<bool>,
    /// Emitted with the column id whose sort was toggled.
    on_sort: EventHandler<String>,
    /// Emitted with `(column_id, width)` during a resize.
    on_resize: EventHandler<(String, u32)>,
    /// Emitted with `(column_id, value)` when a filter changes.
    on_filter_change: EventHandler<(String, FilterValue)>,
    /// Emitted when the user asks for all filters to be dropped.
    on_clear_filters: EventHandler<()>,
) -> Element {
    let visible = visible_columns(&columns).cloned().collect::<Vec<_>>();
    let show_filter_row = has_filter_row(&columns);

    // `<input type="checkbox">` cannot express "indeterminate" declaratively,
    // so the select-all control is a button carrying the checkbox role and a
    // proper tri-state `aria-checked`.
    let all_checked_state = if all_selected {
        "true"
    } else if some_selected {
        "mixed"
    } else {
        "false"
    };
    let all_checked_mark = if all_selected {
        "✓"
    } else if some_selected {
        "–"
    } else {
        ""
    };

    let header_cells = visible.iter().cloned().map(|col| {
        let id = col.id.as_str().to_string();
        let title = col.title.clone();
        let width = col.width;
        let current_width = column_widths.get_clamped(&id, width.current, width.min, width.max);
        let aria_sort = sort_state.aria_sort_for(&id);
        let align_class = match col.align {
            ColumnAlign::Left => "text-left justify-start",
            ColumnAlign::Center => "text-center justify-center",
            ColumnAlign::Right => "text-right justify-end",
        };
        let glyph = match sort_state.is_sorted_by(&id) {
            Some(SortDirection::Asc) => "▲",
            Some(SortDirection::Desc) => "▼",
            None => "⇅",
        };

        let control: Element = if col.is_checkbox() {
            rsx! {
                button {
                    r#type: "button",
                    role: "checkbox",
                    "aria-checked": all_checked_state,
                    "aria-label": "Select all rows on this page",
                    class: "h-4 w-4 rounded border border-border flex items-center justify-center text-[10px] leading-none cursor-pointer text-primary focus:outline-none focus-visible:ring-1 focus-visible:ring-primary/50",
                    onclick: move |ev| {
                        ev.stop_propagation();
                        on_toggle_all.call(!all_selected);
                    },
                    span { "aria-hidden": "true", "{all_checked_mark}" }
                }
            }
        } else if col.sortable {
            // A real <button>: focusable, activated with Enter/Space and
            // announced as a control by screen readers.
            let sort_id = id.clone();
            let sort_title = title.clone();
            let label = title.clone();
            rsx! {
                button {
                    r#type: "button",
                    class: "cursor-pointer hover:text-foreground flex items-center gap-1 transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-primary/50 rounded",
                    title: "Sort by {sort_title}",
                    onclick: move |_| on_sort.call(sort_id.clone()),
                    span { "{label}" }
                    span {
                        class: "inline-flex text-[10px] text-muted-foreground/70",
                        "aria-hidden": "true",
                        "{glyph}"
                    }
                }
            }
        } else {
            let label = title.clone();
            rsx! {
                div { class: "flex items-center gap-1",
                    span { "{label}" }
                }
            }
        };

        let resize_handle: Option<Element> = col.resizable.then(|| {
            rsx! {
                ColumnResizeHandle {
                    column_id: id.clone(),
                    column_title: Some(title.clone()),
                    current_width: current_width,
                    min_width: width.min,
                    max_width: width.max,
                    on_resize: on_resize,
                }
            }
        });

        rsx! {
            th {
                key: "{id}",
                scope: "col",
                class: "relative px-3 py-2.5 font-semibold text-foreground/80 tracking-wide border-r border-border/40 last:border-r-0 group/th",
                style: "width: {current_width}px; min-width: {width.min}px; max-width: {width.max}px;",
                "aria-sort": aria_sort,
                div { class: "flex items-center gap-1.5 {align_class}", {control} }
                {resize_handle}
            }
        }
    });

    let filter_cells = visible.iter().cloned().map(|col| {
        let id = col.id.as_str().to_string();
        let value = filters.get(&id).cloned();
        let filter_type = col.filter_type.clone().filter(|_| col.has_filter());

        let control: Element = if col.is_checkbox() {
            rsx! {
                button {
                    r#type: "button",
                    title: "Reset all filters",
                    "aria-label": "Reset all filters",
                    class: "text-[10px] text-muted-foreground hover:text-foreground px-1 py-0.5 rounded hover:bg-muted font-medium transition-colors",
                    onclick: move |_| on_clear_filters.call(()),
                    "✕"
                }
            }
        } else if let Some(filter_type) = filter_type {
            rsx! {
                GridFilterCell {
                    column_id: id.clone(),
                    column_title: Some(col.title.clone()),
                    filter_type: filter_type,
                    current_value: value,
                    commit_mode: filter_commit,
                    on_change: on_filter_change,
                }
            }
        } else {
            rsx! {
                div { class: "h-6" }
            }
        };

        rsx! {
            td {
                key: "{id}",
                class: "px-2 py-1.5 border-r border-border/40 last:border-r-0 align-middle",
                {control}
            }
        }
    });

    let filter_row: Option<Element> = show_filter_row.then(|| {
        rsx! {
            tr { class: "bg-muted/30 border-t border-border/40", {filter_cells} }
        }
    });

    rsx! {
        thead { class: "bg-muted/50 text-xs uppercase sticky top-0 z-10",
            tr { class: "border-b border-border", {header_cells} }
            {filter_row}
        }
    }
}
