use leptos::ev::MouseEvent;
use leptos::prelude::*;

use rustok_grid::{ColumnAlign, ColumnWidths, GridColumnDef};

#[component]
pub fn GridRow<T: Send + Sync + Clone + 'static>(
    item: T,
    row_id: String,
    columns: Vec<GridColumnDef>,
    column_widths: Signal<ColumnWidths>,
    is_selected: Signal<bool>,
    on_toggle_select: Callback<String>,
    on_row_click: Option<Callback<T>>,
    cell_renderer: Callback<(T, String), AnyView>,
) -> impl IntoView {
    let id_for_select_toggle = row_id.clone();
    let item_for_click = item.clone();

    view! {
        <tr
            class=move || {
                let base = "border-b border-border/60 hover:bg-muted/40 transition-colors text-sm text-foreground";
                if is_selected.get() {
                    format!("{base} bg-primary/5")
                } else {
                    base.to_string()
                }
            }
        >
            {columns
                .into_iter()
                .filter(|col| col.visible)
                .map(|col| {
                    let col_id = col.id.0.clone();
                    let is_checkbox = col_id == "__checkbox";
                    let align_class = match col.align {
                        ColumnAlign::Left => "text-left",
                        ColumnAlign::Center => "text-center",
                        ColumnAlign::Right => "text-right",
                    };

                    let current_w = {
                        let id_for_w = col_id.clone();
                        move || column_widths.get().get(&id_for_w, col.width.current)
                    };

                    let item_c = item_for_click.clone();
                    let id_c = id_for_select_toggle.clone();
                    let rendered_cell = if is_checkbox {
                        view! {
                            <input
                                type="checkbox"
                                prop:checked=move || is_selected.get()
                                on:click=move |ev| ev.stop_propagation()
                                on:change=move |_| on_toggle_select.run(id_c.clone())
                                class="rounded border-border text-primary focus:ring-primary/30 h-4 w-4 cursor-pointer"
                            />
                        }
                        .into_any()
                    } else {
                        cell_renderer.run((item.clone(), col_id))
                    };

                    view! {
                        <td
                            class=format!("px-3 py-2.5 border-r border-border/30 last:border-r-0 align-middle {align_class}")
                            style=move || format!("width: {}px; min-width: {}px; max-width: {}px;", current_w(), col.width.min, col.width.max)
                            on:click=move |_ev: MouseEvent| {
                                if !is_checkbox {
                                    if let Some(on_click) = on_row_click {
                                        on_click.run(item_c.clone());
                                    }
                                }
                            }
                        >
                            {rendered_cell}
                        </td>
                    }
                    .into_any()
                })
                .collect_view()}
        </tr>
    }
}
