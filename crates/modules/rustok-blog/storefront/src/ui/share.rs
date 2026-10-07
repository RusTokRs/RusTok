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
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let is_ru = locale.as_deref() == Some("ru");
    let share_label = if is_ru {
        "Поделиться"
    } else {
        "Share"
    };
    let telegram_label = "Telegram";
    let vk_label = "VK";
    let twitter_label = "X (Twitter)";

    let article_url = format!("/blog?slug={}", core::percent_encode(&slug));
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
