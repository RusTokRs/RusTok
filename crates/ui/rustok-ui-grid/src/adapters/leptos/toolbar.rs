use leptos::prelude::*;

use crate::core::GridColumnDef;

#[component]
pub fn GridToolbar(
    total_count: Signal<u64>,
    selected_count: Signal<usize>,
    has_active_filters: Signal<bool>,
    on_clear_filters: Callback<()>,
    columns: Signal<Vec<GridColumnDef>>,
    on_toggle_column_visibility: Callback<String>,
    bulk_actions: Option<Callback<usize, AnyView>>,
) -> impl IntoView {
    let (show_columns_dropdown, set_show_columns_dropdown) = signal(false);

    view! {
        <div class="flex items-center justify-between gap-3 px-4 py-3 border-b border-border/80 bg-background text-sm">
            // Left: Total count or Selection banner
            <div class="flex items-center gap-3">
                {move || {
                    let sel = selected_count.get();
                    if sel > 0 {
                        view! {
                            <div class="flex items-center gap-2 px-2.5 py-1 rounded bg-primary/10 text-primary text-xs font-semibold">
                                <span>{sel} " selected"</span>
                                {if let Some(actions) = bulk_actions {
                                    actions.run(sel)
                                } else {
                                    ().into_any()
                                }}
                            </div>
                        }
                        .into_any()
                    } else {
                        view! {
                            <div class="flex items-center gap-2 text-muted-foreground text-xs">
                                <span>"Total items:"</span>
                                <span class="font-bold text-foreground">{total_count}</span>
                            </div>
                        }
                        .into_any()
                    }
                }}

                // Active filters badge + reset
                {move || {
                    if has_active_filters.get() {
                        view! {
                            <button
                                type="button"
                                on:click=move |_| on_clear_filters.run(())
                                class="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full bg-destructive/10 text-destructive hover:bg-destructive/20 transition-colors font-medium"
                            >
                                <span>"Filters active"</span>
                                <span class="text-[10px]">"✕"</span>
                            </button>
                        }
                        .into_any()
                    } else {
                        ().into_any()
                    }
                }}
            </div>

            // Right: Column visibility picker
            <div class="relative">
                <button
                    type="button"
                    on:click=move |_| set_show_columns_dropdown.update(|open| *open = !*open)
                    class="text-xs px-2.5 py-1 rounded border border-border bg-background hover:bg-muted font-medium text-foreground transition-colors flex items-center gap-1.5 shadow-sm"
                >
                    <span>"Columns ⚙"</span>
                </button>

                {move || {
                    if show_columns_dropdown.get() {
                        view! {
                            <div
                                class="absolute right-0 mt-1 w-48 rounded-lg border border-border bg-popover text-popover-foreground shadow-lg p-2 z-30 text-xs space-y-1"
                            >
                                <div class="font-semibold text-muted-foreground px-1 pb-1 border-b border-border/50">
                                    "Toggle Columns"
                                </div>
                                <div class="max-h-56 overflow-y-auto space-y-1 pt-1">
                                    {columns.get()
                                        .into_iter()
                                        .filter(|col| col.id.0 != "__checkbox" && col.pinned.is_none())
                                        .map(|col| {
                                            let id = col.id.0.clone();
                                            let id_for_toggle = id.clone();
                                            let visible = col.visible;
                                            view! {
                                                <label class="flex items-center gap-2 px-1 py-0.5 rounded hover:bg-muted/60 cursor-pointer">
                                                    <input
                                                        type="checkbox"
                                                        prop:checked=visible
                                                        on:change=move |_| on_toggle_column_visibility.run(id_for_toggle.clone())
                                                        class="rounded border-border text-primary focus:ring-primary/20 h-3.5 w-3.5"
                                                    />
                                                    <span class="truncate">{col.title}</span>
                                                </label>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </div>
                        }
                        .into_any()
                    } else {
                        ().into_any()
                    }
                }}
            </div>
        </div>
    }
}
