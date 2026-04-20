mod defaults;
mod diagnostics;
mod redirects;
mod robots;
mod sitemaps;

use leptos::prelude::*;

use crate::i18n::t;
use crate::model::SeoAdminTab;

pub use defaults::SeoDefaultsPane;
pub use diagnostics::SeoDiagnosticsPane;
pub use redirects::SeoRedirectsPane;
pub use robots::SeoRobotsPane;
pub use sitemaps::SeoSitemapsPane;

#[component]
pub fn SeoAdminHeader(
    ui_locale: Option<String>,
    status_message: RwSignal<Option<String>>,
) -> impl IntoView {
    let badge_text = t(ui_locale.as_deref(), "seo.badge", "seo");
    let title_text = t(ui_locale.as_deref(), "seo.title", "SEO Hub");
    let subtitle_text = t(
        ui_locale.as_deref(),
        "seo.subtitle",
        "Cross-cutting SEO control plane for tenant defaults, redirects, robots, and sitemaps. Entity SEO authoring now lives in owner-module editors.",
    );

    view! {
        <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
            <p class="text-xs font-semibold uppercase tracking-[0.2em] text-muted-foreground">{badge_text}</p>
            <h1 class="mt-2 text-3xl font-semibold text-card-foreground">{title_text}</h1>
            <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{subtitle_text}</p>
            <Show when=move || status_message.get().is_some()>
                <p class="mt-4 rounded-lg bg-secondary px-3 py-2 text-sm text-secondary-foreground">
                    {move || status_message.get().unwrap_or_default()}
                </p>
            </Show>
        </section>
    }
}

#[component]
pub fn SeoAdminTabs(
    ui_locale: Option<String>,
    active_tab: Signal<SeoAdminTab>,
    on_select: Callback<SeoAdminTab>,
) -> impl IntoView {
    let redirects_label = t(ui_locale.as_deref(), "seo.tab.redirects", "Redirects");
    let sitemaps_label = t(ui_locale.as_deref(), "seo.tab.sitemaps", "Sitemaps");
    let robots_label = t(ui_locale.as_deref(), "seo.tab.robots", "Robots");
    let defaults_label = t(ui_locale.as_deref(), "seo.tab.defaults", "Defaults");
    let diagnostics_label = t(ui_locale.as_deref(), "seo.tab.diagnostics", "Diagnostics");

    view! {
        <nav class="flex flex-wrap gap-3">
            <SeoAdminTabButton
                label=redirects_label
                tab=SeoAdminTab::Redirects
                active_tab=active_tab
                on_select=on_select
            />
            <SeoAdminTabButton
                label=sitemaps_label
                tab=SeoAdminTab::Sitemaps
                active_tab=active_tab
                on_select=on_select
            />
            <SeoAdminTabButton
                label=robots_label
                tab=SeoAdminTab::Robots
                active_tab=active_tab
                on_select=on_select
            />
            <SeoAdminTabButton
                label=defaults_label
                tab=SeoAdminTab::Defaults
                active_tab=active_tab
                on_select=on_select
            />
            <SeoAdminTabButton
                label=diagnostics_label
                tab=SeoAdminTab::Diagnostics
                active_tab=active_tab
                on_select=on_select
            />
        </nav>
    }
}

#[component]
fn SeoAdminTabButton(
    label: String,
    tab: SeoAdminTab,
    active_tab: Signal<SeoAdminTab>,
    on_select: Callback<SeoAdminTab>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            class=move || {
                if active_tab.get() == tab {
                    "rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground"
                } else {
                    "rounded-full border border-border px-4 py-2 text-sm font-medium text-foreground transition hover:bg-accent"
                }
            }
            on:click=move |_| on_select.run(tab)
        >
            {label}
        </button>
    }
}

#[component]
pub fn SeoBusyFooter(busy_key: RwSignal<Option<String>>) -> impl IntoView {
    view! {
        <Show when=move || busy_key.get().is_some()>
            <p class="text-sm text-muted-foreground">
                {move || format!("Working: {}", busy_key.get().unwrap_or_default())}
            </p>
        </Show>
    }
}
