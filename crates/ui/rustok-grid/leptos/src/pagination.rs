use leptos::ev::Event;
use leptos::html::Div;
use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos_use::use_intersection_observer;

use rustok_grid::{GridPagination, PaginationMode};

/// Default choices offered by the "rows per page" selector.
pub const DEFAULT_PAGE_SIZE_OPTIONS: [usize; 4] = [10, 20, 50, 100];

/// Footer bar: range summary, page size, mode switch, page navigation and the
/// infinite-scroll sentinel.
#[component]
pub fn GridPaginationBar(
    pagination: Signal<GridPagination>,
    on_page_change: Callback<usize>,
    on_page_size_change: Callback<usize>,
    on_mode_change: Callback<PaginationMode>,
    on_load_more: Callback<()>,
    /// Selectable page sizes. The current page size is always included so the
    /// `<select>` can never end up showing a blank value.
    #[prop(optional)]
    page_size_options: Option<Vec<usize>>,
) -> impl IntoView {
    let sentinel_ref = NodeRef::<Div>::new();
    let configured_options =
        StoredValue::new(page_size_options.unwrap_or_else(|| DEFAULT_PAGE_SIZE_OPTIONS.to_vec()));

    // Infinite scroll: load the next page when the sentinel becomes visible.
    #[cfg(target_arch = "wasm32")]
    let _ = use_intersection_observer(sentinel_ref, move |entries, _| {
        if !entries.iter().any(|entry| entry.is_intersecting()) {
            return;
        }
        let state = pagination.get_untracked();
        if state.mode == PaginationMode::Infinite && state.has_next {
            on_load_more.run(());
        }
    });

    #[cfg(not(target_arch = "wasm32"))]
    let _ = on_load_more;

    let current_page = move || pagination.get().page;
    let total_pages = move || pagination.get().total_pages();
    let total_items = move || pagination.get().total;
    let from_item = move || pagination.get().from_index();
    let to_item = move || pagination.get().to_index();
    let page_size = move || pagination.get().page_size;
    let has_prev = move || pagination.get().has_previous();
    let has_next = move || pagination.get().has_next;
    let is_infinite = move || pagination.get().mode == PaginationMode::Infinite;

    let size_options = move || {
        let mut options = configured_options.get_value();
        options.retain(|size| *size > 0);
        let current = page_size();
        if !options.contains(&current) {
            options.push(current);
        }
        options.sort_unstable();
        options.dedup();
        options
    };

    view! {
        <div class="flex flex-col sm:flex-row items-center justify-between gap-3 px-4 py-3 border-t border-border/80 bg-muted/20 text-xs text-muted-foreground select-none">
            // Range summary & page size
            <div class="flex items-center gap-3">
                <div class="flex items-center gap-1.5" aria-live="polite">
                    <span>"Showing"</span>
                    <span class="font-semibold text-foreground">{from_item} "-" {to_item}</span>
                    <span>"of"</span>
                    <span class="font-semibold text-foreground">{total_items}</span>
                </div>

                <div class="h-3 w-px bg-border/60" aria-hidden="true"></div>

                <label class="flex items-center gap-1.5">
                    <span>"Rows per page:"</span>
                    <select
                        prop:value=move || page_size().to_string()
                        on:change=move |ev: Event| {
                            if let Ok(size) = event_target_value(&ev).parse::<usize>()
                                && size > 0
                            {
                                on_page_size_change.run(size);
                            }
                        }
                        class="text-xs rounded border border-border bg-background px-1.5 py-0.5 text-foreground focus:border-primary focus:outline-none"
                    >
                        {move || {
                            let current = page_size();
                            size_options()
                                .into_iter()
                                .map(|size| {
                                    view! {
                                        <option value=size.to_string() selected=size == current>
                                            {size.to_string()}
                                        </option>
                                    }
                                })
                                .collect_view()
                        }}
                    </select>
                </label>
            </div>

            // Mode switch & page navigation
            <nav class="flex items-center gap-2" aria-label="Pagination">
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

                <Show when=move || !is_infinite()>
                    <div class="flex items-center gap-1 ml-2">
                        <button
                            type="button"
                            aria-label="Previous page"
                            disabled=move || !has_prev()
                            on:click=move |_| {
                                let page = current_page();
                                if page > 1 {
                                    on_page_change.run(page - 1);
                                }
                            }
                            class="px-2 py-1 rounded border border-border disabled:opacity-40 disabled:cursor-not-allowed hover:bg-muted font-medium text-foreground transition-colors"
                        >
                            "‹ Previous"
                        </button>
                        <span class="px-2 font-medium text-foreground">
                            {move || current_page()} " / " {move || total_pages()}
                        </span>
                        <button
                            type="button"
                            aria-label="Next page"
                            disabled=move || !has_next()
                            on:click=move |_| {
                                if has_next() {
                                    on_page_change.run(current_page().saturating_add(1));
                                }
                            }
                            class="px-2 py-1 rounded border border-border disabled:opacity-40 disabled:cursor-not-allowed hover:bg-muted font-medium text-foreground transition-colors"
                        >
                            "Next ›"
                        </button>
                    </div>
                </Show>
            </nav>

            // Sentinel observed for infinite scrolling.
            <Show when=move || is_infinite()>
                <div
                    node_ref=sentinel_ref
                    class="h-1 w-full -mt-2 opacity-0 pointer-events-none"
                    aria-hidden="true"
                />
            </Show>
        </div>
    }
}
