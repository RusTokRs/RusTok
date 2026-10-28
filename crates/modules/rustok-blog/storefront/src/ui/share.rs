use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::core;

fn public_origin() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        return web_sys::window()?.location().origin().ok();
    }

    #[cfg(all(feature = "ssr", not(target_arch = "wasm32")))]
    {
        let configured = std::env::var("NEXT_PUBLIC_STOREFRONT_URL")
            .or_else(|_| std::env::var("NEXT_PUBLIC_SITE_URL"))
            .ok()?;
        let parsed = url::Url::parse(&configured).ok()?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return None;
        }
        return Some(parsed.origin().ascii_serialization());
    }

    #[cfg(all(not(feature = "ssr"), not(target_arch = "wasm32")))]
    {
        None
    }
}

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
    let article_path = core::module_href(&route_base, &core::percent_encode(&slug));
    // External sharing APIs require a fully qualified URL. Missing public
    // origin is not a reason to publish a broken relative share target.
    let Some(origin) = public_origin() else {
        return ().into_any();
    };
    let article_url = format!("{}{}", origin.trim_end_matches('/'), article_path);
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
    }.into_any()
}
