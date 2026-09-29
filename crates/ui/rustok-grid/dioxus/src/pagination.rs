//! Footer bar: range summary, page size, mode switch and page navigation.

use dioxus::prelude::*;

use rustok_grid::{GridPagination, PaginationMode};

/// Default choices offered by the "rows per page" selector.
pub const DEFAULT_PAGE_SIZE_OPTIONS: [usize; 4] = [10, 20, 50, 100];

/// Footer bar of the grid.
///
/// In infinite mode the next page is requested when the sentinel scrolls into
/// view (`onvisible`, an intersection observer provided by Dioxus itself) — and
/// there is always a real button next to it, because "scroll to load" is
/// unreachable by keyboard.
#[component]
pub fn GridPaginationBar(
    /// Pagination state as it should be rendered.
    pagination: GridPagination,
    /// Emitted with the requested page number.
    on_page_change: EventHandler<usize>,
    /// Emitted with the requested page size.
    on_page_size_change: EventHandler<usize>,
    /// Emitted when the user switches between paged and infinite mode.
    on_mode_change: EventHandler<PaginationMode>,
    /// Emitted when the next chunk should be appended in infinite mode.
    on_load_more: EventHandler<()>,
    /// Selectable page sizes. The current page size is always included so the
    /// `<select>` can never end up showing a blank value.
    #[props(default)]
    page_size_options: Option<Vec<usize>>,
) -> Element {
    let current_page = pagination.page;
    let total_pages = pagination.total_pages();
    let total_items = pagination.total;
    let from_item = pagination.from_index();
    let to_item = pagination.to_index();
    let page_size = pagination.page_size;
    let has_prev = pagination.has_previous();
    let has_next = pagination.has_next;
    let is_infinite = pagination.mode == PaginationMode::Infinite;

    let size_options = {
        let mut options = page_size_options.unwrap_or_else(|| DEFAULT_PAGE_SIZE_OPTIONS.to_vec());
        options.retain(|size| *size > 0);
        if !options.contains(&page_size) {
            options.push(page_size);
        }
        options.sort_unstable();
        options.dedup();
        options
    };

    let pager: Option<Element> = (!is_infinite).then(|| {
        rsx! {
            div { class: "flex items-center gap-1 ml-2",
                button {
                    r#type: "button",
                    "aria-label": "Previous page",
                    disabled: !has_prev,
                    class: "px-2 py-1 rounded border border-border disabled:opacity-40 disabled:cursor-not-allowed hover:bg-muted font-medium text-foreground transition-colors",
                    onclick: move |_| {
                        if current_page > 1 {
                            on_page_change.call(current_page - 1);
                        }
                    },
                    "‹ Previous"
                }
                span { class: "px-2 font-medium text-foreground", "{current_page} / {total_pages}" }
                button {
                    r#type: "button",
                    "aria-label": "Next page",
                    disabled: !has_next,
                    class: "px-2 py-1 rounded border border-border disabled:opacity-40 disabled:cursor-not-allowed hover:bg-muted font-medium text-foreground transition-colors",
                    onclick: move |_| {
                        if has_next {
                            on_page_change.call(current_page.saturating_add(1));
                        }
                    },
                    "Next ›"
                }
            }
        }
    });

    let loader: Option<Element> = (is_infinite && has_next).then(|| {
        rsx! {
            button {
                r#type: "button",
                class: "text-[11px] px-2 py-1 rounded border border-border hover:bg-muted font-medium text-foreground transition-colors",
                onclick: move |_| on_load_more.call(()),
                "Load more"
            }
            // Sentinel: Dioxus wires this up to an intersection observer, so no
            // JavaScript and no `web-sys` are involved.
            div {
                class: "h-1 w-full -mt-2 opacity-0 pointer-events-none",
                "aria-hidden": "true",
                onvisible: move |ev| {
                    if ev.data().is_intersecting().unwrap_or(false) {
                        on_load_more.call(());
                    }
                },
            }
        }
    });

    rsx! {
        div {
            class: "flex flex-col sm:flex-row items-center justify-between gap-3 px-4 py-3 border-t border-border/80 bg-muted/20 text-xs text-muted-foreground select-none",

            // Range summary & page size
            div { class: "flex items-center gap-3",
                div { class: "flex items-center gap-1.5", "aria-live": "polite",
                    span { "Showing" }
                    span { class: "font-semibold text-foreground", "{from_item}-{to_item}" }
                    span { "of" }
                    span { class: "font-semibold text-foreground", "{total_items}" }
                }
                div { class: "h-3 w-px bg-border/60", "aria-hidden": "true" }
                label { class: "flex items-center gap-1.5",
                    span { "Rows per page:" }
                    select {
                        value: "{page_size}",
                        class: "text-xs rounded border border-border bg-background px-1.5 py-0.5 text-foreground focus:border-primary focus:outline-none",
                        onchange: move |ev| {
                            if let Ok(size) = ev.value().parse::<usize>() {
                                if size > 0 {
                                    on_page_size_change.call(size);
                                }
                            }
                        },
                        for size in size_options {
                            option {
                                key: "{size}",
                                value: "{size}",
                                selected: size == page_size,
                                "{size}"
                            }
                        }
                    }
                }
            }

            // Mode switch & page navigation
            nav { class: "flex items-center gap-2", "aria-label": "Pagination",
                button {
                    r#type: "button",
                    class: "text-[11px] px-2 py-1 rounded border border-border hover:bg-muted font-medium transition-colors text-foreground/80 flex items-center gap-1",
                    title: "Toggle between classic pagination and infinite scrolling",
                    onclick: move |_| {
                        let next_mode = if is_infinite {
                            PaginationMode::Paged
                        } else {
                            PaginationMode::Infinite
                        };
                        on_mode_change.call(next_mode);
                    },
                    if is_infinite {
                        "Mode: Infinite ⤓"
                    } else {
                        "Mode: Paged 📄"
                    }
                }
                {pager}
                {loader}
            }
        }
    }
}
