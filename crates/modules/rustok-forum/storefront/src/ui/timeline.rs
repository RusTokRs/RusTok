use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::i18n::{count_message, t};

/// Read-progress rail for a forum thread.
///
/// `loaded_posts` is how many posts the reader actually has in the document (the topic plus every
/// reply page appended so far) and `total_posts` is the owner-authored thread size. The filled
/// share is therefore "how far the loaded window reaches", never a pretend cursor position inside
/// posts the reader has not fetched. The jump controls are real in-document anchors: they used to
/// be buttons with no handler, which read as a working control and did nothing.
#[component]
pub fn TimelineScroller(
    #[prop(default = 1)] loaded_posts: u64,
    #[prop(default = 1)] total_posts: u64,
) -> impl IntoView {
    if total_posts <= 1 {
        return view! { <div class="hidden"></div> }.into_any();
    }

    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let to_top_label = t(
        locale.as_deref(),
        "forum.timeline.toTop",
        "Jump to the first post",
    );
    let to_end_label = t(
        locale.as_deref(),
        "forum.timeline.toEnd",
        "Jump to the newest post",
    );
    let loaded_hint = count_message(
        locale.as_deref(),
        "forum.timeline.loaded",
        "{count} posts loaded",
        loaded_posts,
    );
    let total_hint = count_message(
        locale.as_deref(),
        "forum.timeline.total",
        "{count} posts in this thread",
        total_posts,
    );
    let progress_hint = format!("{loaded_hint} \u{00b7} {total_hint}");

    let percentage = if total_posts > 0 {
        ((loaded_posts as f64 / total_posts as f64) * 100.0).clamp(0.0, 100.0) as u64
    } else {
        0
    };

    view! {
        <aside
            class="sticky top-20 flex flex-col items-center gap-2 rounded-2xl border border-border bg-card/80 p-2 shadow-sm backdrop-blur"
            title=progress_hint.clone()
        >
            <a
                href="#forum-thread-top"
                class="rounded-xl p-1.5 text-xs text-muted-foreground transition hover:bg-muted hover:text-foreground"
                title=to_top_label.clone()
                aria-label=to_top_label
            >
                "\u{2191}"
            </a>

            <div class="flex flex-col items-center py-1">
                <span class="text-xs font-bold text-foreground">{loaded_posts}</span>
                <div
                    class="my-1.5 h-16 w-1 overflow-hidden rounded-full bg-muted"
                    role="progressbar"
                    aria-valuenow=percentage.to_string()
                    aria-valuemin="0"
                    aria-valuemax="100"
                    aria-label=progress_hint.clone()
                >
                    <svg
                        aria-hidden="true"
                        class="block h-full w-full"
                        preserveAspectRatio="none"
                        viewBox="0 0 4 100"
                    >
                        <rect
                            class="fill-primary transition-all duration-300"
                            x="0"
                            y="0"
                            width="4"
                            height=format!("{percentage}")
                        />
                    </svg>
                </div>
                <span class="text-[10px] text-muted-foreground">{total_posts}</span>
            </div>

            <a
                href="#forum-thread-end"
                class="rounded-xl p-1.5 text-xs text-muted-foreground transition hover:bg-muted hover:text-foreground"
                title=to_end_label.clone()
                aria-label=to_end_label
            >
                "\u{2193}"
            </a>
        </aside>
    }
    .into_any()
}
