//! Footer bar: range summary, page size, mode switch and page navigation.

use dioxus::prelude::*;

use rustok_grid::{GridPagination, PaginationMode};

/// The choices offered by the "rows per page" selector when the caller does not configure any.
///
/// Re-exported from the core rather than repeated here: the picker and the state machine that
/// validates a page size must agree on the list, and only the core can guarantee that.
pub use rustok_grid::DEFAULT_PAGE_SIZE_OPTIONS;

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
    let messages = try_consume_context::<rustok_grid::GridMessages>().unwrap_or_default();
    let current_page = pagination.page;
    let total_pages = pagination.total_pages();
    let total_items = pagination.total;
    let from_item = pagination.from_index();
    let to_item = pagination.to_index();
    let page_size = pagination.page_size;
    let has_prev = pagination.has_previous();
    let has_next = pagination.has_next;
    let is_infinite = pagination.mode == PaginationMode::Infinite;

    // An unset list stays empty; the core's normalizer turns "unset" into the default choices and
    // keeps the current size selectable, which is the same rule the Leptos bar follows.
    let size_options = GridPagination::normalize_page_size_options(
        &page_size_options.unwrap_or_default(),
        page_size,
    );

    let pager_messages = messages.clone();
    let pager: Option<Element> = (!is_infinite).then(|| {
        let messages = &pager_messages;
        rsx! {
            div { class: "flex items-center gap-1 ml-2",
                button {
                    r#type: "button",
                    "aria-label": messages.text("grid.previous"),
                    disabled: !has_prev,
                    class: "px-2 py-1 rounded border border-border disabled:opacity-40 disabled:cursor-not-allowed hover:bg-muted font-medium text-foreground transition-colors",
                    onclick: move |_| {
                        if current_page > 1 {
                            on_page_change.call(current_page - 1);
                        }
                    },
                    {messages.text("grid.previous")}
                }
                span { class: "px-2 font-medium text-foreground", "{current_page} / {total_pages}" }
                button {
                    r#type: "button",
                    "aria-label": messages.text("grid.next"),
                    disabled: !has_next,
                    class: "px-2 py-1 rounded border border-border disabled:opacity-40 disabled:cursor-not-allowed hover:bg-muted font-medium text-foreground transition-colors",
                    onclick: move |_| {
                        if has_next {
                            on_page_change.call(current_page.saturating_add(1));
                        }
                    },
                    {messages.text("grid.next")}
                }
            }
        }
    });

    let loader_messages = messages.clone();
    let loader: Option<Element> = (is_infinite && has_next).then(|| {
        let messages = &loader_messages;
        rsx! {
            button {
                r#type: "button",
                // The visible text alone does not say what is loaded; the Leptos bar names the
                // same action, so both hosts announce it identically.
                "aria-label": messages.text("grid.loadMore"),
                class: "text-[11px] px-2 py-1 rounded border border-border hover:bg-muted font-medium text-foreground transition-colors",
                onclick: move |_| on_load_more.call(()),
                {messages.text("grid.loadMore")}
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
                    {messages.range(from_item, to_item, total_items)}
                }
                div { class: "h-3 w-px bg-border/60", "aria-hidden": "true" }
                label { class: "flex items-center gap-1.5",
                    span { {messages.text("grid.rowsPerPage")} }
                    select {
                        value: "{page_size}",
                        class: "text-xs rounded border border-border bg-background px-1.5 py-0.5 text-foreground focus:border-primary focus:outline-none",
                        onchange: move |ev| {
                            if let Ok(size) = ev.value().parse::<usize>()
                                && size > 0
                            {
                                on_page_size_change.call(size);
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
            nav { class: "flex items-center gap-2", "aria-label": messages.text("grid.pagination"),
                button {
                    r#type: "button",
                    class: "text-[11px] px-2 py-1 rounded border border-border hover:bg-muted font-medium transition-colors text-foreground/80 flex items-center gap-1",
                    title: messages.text("grid.modeHint"),
                    onclick: move |_| {
                        let next_mode = if is_infinite {
                            PaginationMode::Paged
                        } else {
                            PaginationMode::Infinite
                        };
                        on_mode_change.call(next_mode);
                    },
                    if is_infinite {
                        {messages.text("grid.modeInfinite")}
                    } else {
                        {messages.text("grid.modePaged")}
                    }
                }
                {pager}
                {loader}
            }
        }
    }
}
