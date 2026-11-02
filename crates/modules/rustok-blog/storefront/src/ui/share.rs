use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::core;

// Browser location is the share origin. It must not be guessed from a Next-only
// environment variable during SSR: server and client can have different origins,
// which would also make their initial hydration trees disagree.
fn public_origin() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        return web_sys::window()?.location().origin().ok();
    }
    #[cfg(not(target_arch = "wasm32"))]
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
    let is_ru = locale
        .as_deref()
        .is_some_and(|value| value.starts_with("ru"));
    let share_label = if is_ru {
        "Поделиться"
    } else {
        "Share"
    };

    // Keep the module's actual route contract, not Next's /blog/{slug} URL.
    let route_segment = core::route_segment_or_default(
        route.route_segment.as_ref().cloned(),
        core::DEFAULT_ROUTE_SEGMENT,
    );
    let route_base = route.module_route_base(route_segment.as_str());
    let article_path = core::module_href(&route_base, &core::percent_encode(&slug));

    // SSR and the first hydration render both start with no browser-derived
    // URLs. Effects run on the client after hydration, then reveal valid links.
    // No placeholder link is exposed when the origin is unavailable.
    let share_urls = RwSignal::new(None::<(String, String, String)>);
    Effect::new(move |_| {
        let urls = public_origin().map(|origin| {
            let article_url = format!("{}{}", origin.trim_end_matches('/'), article_path);
            (
                core::telegram_share_url(&article_url, &title),
                core::vk_share_url(&article_url, &title),
                core::twitter_share_url(&article_url, &title),
            )
        });
        share_urls.set(urls);
    });

    view! {
        {move || share_urls.get().map(|(tg_url, vk_url, tw_url)| view! {
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
                    "Telegram"
                </a>
                <a
                    href=vk_url
                    target="_blank"
                    rel="noopener noreferrer"
                    class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground hover:bg-muted transition"
                >
                    "VK"
                </a>
                <a
                    href=tw_url
                    target="_blank"
                    rel="noopener noreferrer"
                    class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground hover:bg-muted transition"
                >
                    "X (Twitter)"
                </a>
            </div>
        })}
    }
}
