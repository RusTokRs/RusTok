use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::core;

#[component]
pub fn BlogStatusBadge(status: String, unknown_label: String) -> impl IntoView {
    let badge_view = core::status_badge_typed_view(status, unknown_label.as_str());
    view! {
        <span class=badge_view.badge_css>
            {badge_view.label}
        </span>
    }
}

#[component]
pub fn BlogShareButtons(title: String, slug: String) -> impl IntoView {
    let route = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route.locale.clone();
    let is_ru = locale.as_deref().is_some_and(|value| value.starts_with("ru"));
    let share_label = if is_ru {
        "Поделиться"
    } else {
        "Share"
    };
    let telegram_label = "Telegram";
    let vk_label = "VK";
    let twitter_label = "X (Twitter)";

    // Build the link from the host route contract; Leptos is served under
    // /modules/blog (with an optional locale prefix), not /blog.
    let route_segment = core::route_segment_or_default(
        route.route_segment.as_ref().cloned(),
        core::DEFAULT_ROUTE_SEGMENT,
    );
    let route_base = route.module_route_base(route_segment.as_str());
    let article_url = core::module_href(&route_base, &core::percent_encode(&slug));
    let tg_url = core::telegram_share_url(&article_url, &title);
    let vk_url = core::vk_share_url(&article_url, &title);
    let tw_url = core::twitter_share_url(&article_url, &title);

    view! {
        <div class="mt-6 flex flex-wrap items-center gap-2 rounded-2xl border border-border bg-card p-4">
            <span class="mr-1 text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                {share_label}
            </span>
            <a
                href=tg_url
                target="_blank"
                rel="noopener noreferrer"
                class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground hover:bg-muted transition"
            >
                {telegram_label}
            </a>
            <a
                href=vk_url
                target="_blank"
                rel="noopener noreferrer"
                class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground hover:bg-muted transition"
            >
                {vk_label}
            </a>
            <a
                href=tw_url
                target="_blank"
                rel="noopener noreferrer"
                class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground hover:bg-muted transition"
            >
                {twitter_label}
            </a>
        </div>
    }
}
