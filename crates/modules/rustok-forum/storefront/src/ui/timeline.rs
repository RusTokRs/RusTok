use leptos::prelude::*;

#[component]
pub fn TimelineScroller(
    #[prop(default = 1)] current_post: u64,
    #[prop(default = 1)] total_posts: u64,
) -> impl IntoView {
    if total_posts <= 1 {
        return view! { <div class="hidden"></div> }.into_any();
    }

    let percentage = if total_posts > 0 {
        ((current_post as f64 / total_posts as f64) * 100.0).clamp(0.0, 100.0) as u64
    } else {
        0
    };

    view! {
        <aside class="sticky top-20 flex flex-col items-center gap-2 rounded-2xl border border-border bg-card/80 p-2 shadow-sm backdrop-blur">
            <button
                type="button"
                class="rounded-xl p-1.5 text-xs text-muted-foreground transition hover:bg-muted hover:text-foreground"
                title="Jump to beginning"
            >
                "↑"
            </button>

            <div class="flex flex-col items-center py-1">
                <span class="text-xs font-bold text-foreground">{current_post}</span>
                <div class="my-1.5 h-16 w-1 overflow-hidden rounded-full bg-muted">
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

            <button
                type="button"
                class="rounded-xl p-1.5 text-xs text-muted-foreground transition hover:bg-muted hover:text-foreground"
                title="Jump to end"
            >
                "↓"
            </button>
        </aside>
    }.into_any()
}
