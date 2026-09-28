//! A single data row.

use dioxus::prelude::*;

use rustok_grid::{ColumnAlign, ColumnWidths, GridColumnDef, visible_columns};

use crate::grid::GridCell;

/// One `<tr>` of the grid.
///
/// The row renders exactly the visible columns, in order, and delegates the
/// content of every non-checkbox cell to `cell_renderer`.
#[component]
pub fn GridRow<T: Clone + PartialEq + 'static>(
    /// The row payload handed back to `cell_renderer` and `on_row_click`.
    item: T,
    /// Stable, unique row identity used for selection.
    row_id: String,
    /// Columns to render, in order. Hidden ones are skipped.
    columns: Vec<GridColumnDef>,
    /// User-applied column widths.
    column_widths: ColumnWidths,
    /// Whether this row is part of the current selection.
    is_selected: bool,
    /// Emitted with the row id when the selection checkbox is toggled.
    on_toggle_select: EventHandler<String>,
    /// Emitted when the row itself is clicked, if the caller opted in.
    #[props(default)]
    on_row_click: Option<EventHandler<T>>,
    /// Renders the content of a single cell.
    cell_renderer: Callback<GridCell<T>, Element>,
) -> Element {
    let clickable = on_row_click.is_some();
    let row_class = if is_selected {
        "border-b border-border/60 hover:bg-muted/40 transition-colors text-sm text-foreground bg-primary/5"
    } else {
        "border-b border-border/60 hover:bg-muted/40 transition-colors text-sm text-foreground"
    };
    let selected_attr = if is_selected { "true" } else { "false" };

    let cells = visible_columns(&columns).cloned().map(|col| {
        let column_id = col.id.as_str().to_string();
        let width = col.width;
        let current_width =
            column_widths.get_clamped(&column_id, width.current, width.min, width.max);
        let align_class = match col.align {
            ColumnAlign::Left => "text-left",
            ColumnAlign::Center => "text-center",
            ColumnAlign::Right => "text-right",
        };
        let is_checkbox = col.is_checkbox();
        let interactive = clickable && !is_checkbox;
        let cursor_class = if interactive { " cursor-pointer" } else { "" };

        let content: Element = if is_checkbox {
            let toggle_id = row_id.clone();
            rsx! {
                input {
                    r#type: "checkbox",
                    "aria-label": "Select row",
                    checked: is_selected,
                    class: "rounded border-border text-primary focus:ring-primary/30 h-4 w-4 cursor-pointer",
                    // Keep the row click handler out of the way: ticking a
                    // checkbox must never also open the record.
                    onclick: move |ev| ev.stop_propagation(),
                    onchange: move |_| on_toggle_select.call(toggle_id.clone()),
                }
            }
        } else {
            cell_renderer.call(GridCell {
                item: item.clone(),
                column_id: column_id.clone(),
                row_id: row_id.clone(),
            })
        };

        let click_item = item.clone();
        rsx! {
            td {
                key: "{column_id}",
                class: "px-3 py-2.5 border-r border-border/30 last:border-r-0 align-middle {align_class}{cursor_class}",
                style: "width: {current_width}px; min-width: {width.min}px; max-width: {width.max}px;",
                onclick: move |_| {
                    if interactive {
                        if let Some(ref handler) = on_row_click {
                            handler.call(click_item.clone());
                        }
                    }
                },
                {content}
            }
        }
    });

    rsx! {
        tr { class: row_class, "data-selected": selected_attr, {cells} }
    }
}
