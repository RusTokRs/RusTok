use leptos::ev::Event;
use leptos::html::Div;
use leptos::prelude::*;
use leptos_use::use_intersection_observer;

use crate::core::{GridPagination, PaginationMode};

#[component]
pub fn GridPaginationBar(
    pagination: Signal<GridPagination>,
    on_page_change: Callback<usize>,
    on_page_size_change: Callback<usize>,
    on_mode_change: Callback<PaginationMode>,
    on_load_more: Callback<()>,
) -> impl IntoView {
    let sentinel_ref = NodeRef::<Div>::new();

    // Infinite scroll observer
    let _ = use_intersection_observer(sentinel_ref, move |entries, _| {
        if let Some(entry) = entries.first() {
            if entry.is_intersecting() {
                let p = pagination.get();
                if p.mode == PaginationMode::Infinite && p.has_next {
                    on_load_more.run(());
                }
            }
        }
    });

    let current_page = move || pagination.get().page;
    let total_pages = move || pagination.get().total_pages().max(1);
    let total_items = move || pagination.get().total;
    let from_item = move || pagination.get().from_index();
    let to_item = move || pagination.get().to_index();
    let page_size = move || pagination.get().page_size;
    let has_prev = move || pagination.get().has_previous();
    let has_next = move || pagination.get().has_next;
    let is_infinite = move || pagination.get().mode == PaginationMode::Infinite;

    view! {
        <div class="flex flex-col sm:flex-row items-center justify-between gap-3 px-4 py-3 border-t border-border/80 bg-muted/20 text-xs text-muted-foreground select-none">
            // Mode toggle & Info
            <div class="flex items-center gap-3">
                <div class="flex items-center gap-1.5">
                    <span>"Showing"</span>
                    <span class="font-semibold text-foreground">
                        {from_item} "-" {to_item}
                    </span>
                    <span>"of"</span>
                    <span class="font-semibold text-foreground">{total_items}</span>
                </div>

                <div class="h-3 w-px bg-border/60"></div>

                // Rows per page
                <div class="flex items-center gap-1.5">
                    <span>"Rows per page:"</span>
                    <select
                        prop:value=page_size
                        on:change=move |ev: Event| {
                            if let Ok(size) = event_target_value(&ev).parse::<usize>() {
                                on_page_size_change.run(size);
                            }
                        }
                        class="text-xs rounded border border-border bg-background px-1.5 py-0.5 text-foreground focus:border-primary focus:outline-none"
                    >
                        <option value="10">"10"</option>
                        <option value="20">"20"</option>
                        <option value="50">"50"</option>
                        <option value="100">"100"</option>
                    </select>
                </div>
            </div>

            // Mode switch & Page navigation
            <div class="flex items-center gap-2">
                // Mode Toggle button
                <button
                    type="button"
                    on:click=move |_| {
                        let next_mode = if is_infinite() {
                            PaginationMode::Paged
                        } else {
                            PaginationMode::Infinite
                        };
                        on_mode_change.run(next_mode);
                    }
                    class="text-[11px] px-2 py-1 rounded border border-border hover:bg-muted font-medium transition-colors text-foreground/80 flex items-center gap-1"
                    title="Toggle between classic pagination and infinite scrolling"
                >
                    {move || if is_infinite() { "Mode: Infinite ⤓" } else { "Mode: Paged 📄" }}
                </button>

                // Paged buttons (shown if Paged mode)
                {move || {
                    if !is_infinite() {
                        let cur = current_page();
                        let tot = total_pages();
                        view! {
                            <div class="flex items-center gap-1 ml-2">
                                <button
                                    type="button"
                                    disabled=move || !has_prev()
                                    on:click=move |_| on_page_change.run(cur - 1)
                                    class="px-2 py-1 rounded border border-border disabled:opacity-40 disabled:cursor-not-allowed hover:bg-muted font-medium text-foreground transition-colors"
                                >
                                    "‹ Previous"
                                </button>
                                <span class="px-2 font-medium text-foreground">
                                    {cur} " / " {tot}
                                </span>
                                <button
                                    type="button"
                                    disabled=move || !has_next()
                                    on:click=move |_| on_page_change.run(cur + 1)
                                    class="px-2 py-1 rounded border border-border disabled:opacity-40 disabled:cursor-not-allowed hover:bg-muted font-medium text-foreground transition-colors"
                                >
                                    "Next ›"
                                </button>
                            </div>
                        }
                        .into_any()
                    } else {
                        ().into_any()
                    }
                }}
            </div>

            // Invisible sentinel div for infinite scroll
            {move || {
                if is_infinite() {
                    view! {
                        <div node_ref=sentinel_ref class="h-1 w-full -mt-2 opacity-0 pointer-events-none" />
                    }
                    .into_any()
                } else {
                    ().into_any()
                }
            }}
        </div>
    }
}
