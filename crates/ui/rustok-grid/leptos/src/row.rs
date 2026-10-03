use leptos::prelude::*;

use rustok_grid::{ColumnAlign, ColumnWidths, GridColumnDef};

/// A single data row. Cells are rendered reactively from the column signal so
/// toggling column visibility or resizing does not rebuild the whole table.
#[component]
pub fn GridRow<T: Send + Sync + Clone + 'static>(
    item: T,
    row_id: String,
    columns: Signal<Vec<GridColumnDef>>,
    column_widths: Signal<ColumnWidths>,
    is_selected: Signal<bool>,
    on_toggle_select: Callback<String>,
    on_row_click: Option<Callback<T>>,
    cell_renderer: Callback<(T, String), AnyView>,
) -> impl IntoView {
    // `StoredValue` keeps the row payload alive for the reactive cell closure
    // without cloning it once per column on every re-render.
    let item = StoredValue::new(item);
    let row_id = StoredValue::new(row_id);
    let clickable = on_row_click.is_some();

    view! {
        <tr
            class=move || {
                let base = "border-b border-border/60 hover:bg-muted/40 transition-colors text-sm text-foreground";
                if is_selected.get() { format!("{base} bg-primary/5") } else { base.to_string() }
            }
            data-selected=move || if is_selected.get() { "true" } else { "false" }
        >
            {move || {
                columns
                    .get()
                    .into_iter()
                    .filter(|col| col.visible)
                    .map(|col| {
                        let col_id = col.id.as_str().to_string();
                        let is_checkbox = col.is_checkbox();
                        let align_class = match col.align {
                            ColumnAlign::Left => "text-left",
                            ColumnAlign::Center => "text-center",
                            ColumnAlign::Right => "text-right",
                        };
                        let width = col.width;
                        let current_width = {
                            let col_id = col_id.clone();
                            move || {
                                column_widths
                                    .get()
                                    .get_clamped(&col_id, width.current, width.min, width.max)
                            }
                        };

                        let cell = if is_checkbox {
                            view! {
                                <input
                                    type="checkbox"
                                    aria-label="Select row"
                                    prop:checked=move || is_selected.get()
                                    on:click=move |ev| ev.stop_propagation()
                                    on:change=move |_| on_toggle_select.run(row_id.get_value())
                                    class="rounded border-border text-primary focus:ring-primary/30 h-4 w-4 cursor-pointer"
                                />
                            }
                                .into_any()
                        } else {
                            cell_renderer.run((item.get_value(), col_id))
                        };

                        let interactive = clickable && !is_checkbox;

                        view! {
                            <td
                                class=format!(
                                    "px-3 py-2.5 border-r border-border/30 last:border-r-0 align-middle {align_class}{}",
                                    if interactive { " cursor-pointer" } else { "" },
                                )
                                style=move || {
                                    format!(
                                        "width: {}px; min-width: {}px; max-width: {}px;",
                                        current_width(),
                                        width.min,
                                        width.max,
                                    )
                                }
                                on:click=move |_| {
                                    if interactive
                                        && let Some(on_click) = on_row_click
                                    {
                                        on_click.run(item.get_value());
                                    }
                                }
                            >
                                {cell}
                            </td>
                        }
                    })
                    .collect_view()
            }}
        </tr>
    }
}
