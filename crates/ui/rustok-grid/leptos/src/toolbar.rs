use leptos::ev::KeyboardEvent;
use leptos::prelude::*;

use rustok_grid::GridColumnDef;

/// Grid toolbar: selection banner with bulk actions, active-filter reset and
/// the column visibility picker.
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

    // Columns the user is allowed to hide: the synthetic checkbox column and
    // pinned columns are structural and always stay visible.
    let toggleable_columns = Signal::derive(move || {
        columns
            .get()
            .into_iter()
            .filter(|col| !col.is_checkbox() && col.pinned.is_none())
            .collect::<Vec<_>>()
    });

    view! {
        <div class="flex items-center justify-between gap-3 px-4 py-3 border-b border-border/80 bg-background text-sm">
            // Left: total count or selection banner
            <div class="flex items-center gap-3">
                {move || {
                    let selected = selected_count.get();
                    if selected > 0 {
                        view! {
                            <div
                                class="flex items-center gap-2 px-2.5 py-1 rounded bg-primary/10 text-primary text-xs font-semibold"
                                aria-live="polite"
                            >
                                <span>{selected} " selected"</span>
                                {match bulk_actions {
                                    Some(actions) => actions.run(selected),
                                    None => ().into_any(),
                                }}
                            </div>
                        }
                            .into_any()
                    } else {
                        view! {
                            <div class="flex items-center gap-2 text-muted-foreground text-xs">
                                <span>"Total items:"</span>
                                <span class="font-bold text-foreground">{move || total_count.get()}</span>
                            </div>
                        }
                            .into_any()
                    }
                }}

                // Active filters badge + reset
                <Show when=move || has_active_filters.get()>
                    <button
                        type="button"
                        on:click=move |_| on_clear_filters.run(())
                        class="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full bg-destructive/10 text-destructive hover:bg-destructive/20 transition-colors font-medium"
                    >
                        <span>"Filters active"</span>
                        <span class="text-[10px]" aria-hidden="true">
                            "✕"
                        </span>
                    </button>
                </Show>
            </div>

            // Right: column visibility picker
            <div
                class="relative"
                on:keydown=move |ev: KeyboardEvent| {
                    if ev.key() == "Escape" && show_columns_dropdown.get_untracked() {
                        ev.stop_propagation();
                        set_show_columns_dropdown.set(false);
                    }
                }
            >
                <button
                    type="button"
                    aria-haspopup="true"
                    aria-expanded=move || if show_columns_dropdown.get() { "true" } else { "false" }
                    on:click=move |_| set_show_columns_dropdown.update(|open| *open = !*open)
                    class="text-xs px-2.5 py-1 rounded border border-border bg-background hover:bg-muted font-medium text-foreground transition-colors flex items-center gap-1.5 shadow-sm"
                >
                    <span>"Columns ⚙"</span>
                </button>

                <Show when=move || show_columns_dropdown.get()>
                    // Backdrop: closes the popover on any outside click without
                    // registering a global document listener.
                    <div
                        class="fixed inset-0 z-20"
                        aria-hidden="true"
                        on:click=move |_| set_show_columns_dropdown.set(false)
                    />
                    <div class="absolute right-0 mt-1 w-48 rounded-lg border border-border bg-popover text-popover-foreground shadow-lg p-2 z-30 text-xs space-y-1">
                        <div class="font-semibold text-muted-foreground px-1 pb-1 border-b border-border/50">
                            "Toggle Columns"
                        </div>
                        <div class="max-h-56 overflow-y-auto space-y-1 pt-1">
                            {move || {
                                toggleable_columns
                                    .get()
                                    .into_iter()
                                    .map(|col| {
                                        let id = col.id.as_str().to_string();
                                        let visible = col.visible;
                                        view! {
                                            <label class="flex items-center gap-2 px-1 py-0.5 rounded hover:bg-muted/60 cursor-pointer">
                                                <input
                                                    type="checkbox"
                                                    prop:checked=visible
                                                    on:change=move |_| on_toggle_column_visibility.run(id.clone())
                                                    class="rounded border-border text-primary focus:ring-primary/20 h-3.5 w-3.5"
                                                />
                                                <span class="truncate">{col.title.clone()}</span>
                                            </label>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </div>
                    </div>
                </Show>
            </div>
        </div>
    }
}
