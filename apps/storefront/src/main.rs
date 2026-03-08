mod modules;
mod shared;

use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
// GlobalAttributes enables id= usage in view! macros.
use leptos::prelude::{ClassAttribute, CollectView, ElementChild, GlobalAttributes, RenderHtml};
use leptos::{component, view, IntoView};
use std::path::PathBuf;

use crate::modules::{components_for_slot, init_modules, StorefrontSlot};
use crate::shared::context::enabled_modules::{
    fetch_enabled_modules, use_enabled_modules, EnabledModulesProvider,
};

include!(concat!(env!("OUT_DIR"), "/i18n/mod.rs"));
use i18n::*;

#[derive(Clone, Debug)]
struct ProductCardData {
    title: &'static str,
    description: &'static str,
    price: &'static str,
    badge: Option<&'static str>,
}

fn featured_products(locale: &str) -> Vec<ProductCardData> {
    match locale {
        "ru" => vec![
            ProductCardData {
                title: "РЎРјР°СЂС‚-Р°РєСЃРµСЃСЃСѓР°СЂС‹",
                description: "Р¤СѓРЅРєС†РёРѕРЅР°Р»СЊРЅС‹Рµ РіР°РґР¶РµС‚С‹ РґР»СЏ РїРѕРІСЃРµРґРЅРµРІРЅРѕР№ Р¶РёР·РЅРё.",
                price: "РѕС‚ 4 990 в‚Ѕ",
                badge: Some("Р›РёРґРµСЂ РїСЂРѕРґР°Р¶"),
            },
            ProductCardData {
                title: "Р­РєРѕР»РѕРіРёС‡РЅР°СЏ РєРѕР»Р»РµРєС†РёСЏ",
                description: "РќР°С‚СѓСЂР°Р»СЊРЅС‹Рµ РјР°С‚РµСЂРёР°Р»С‹ Рё РїСЂРµРјРёР°Р»СЊРЅР°СЏ РѕС‚РґРµР»РєР°.",
                price: "РѕС‚ 2 490 в‚Ѕ",
                badge: None,
            },
            ProductCardData {
                title: "Р“РѕСЂРѕРґСЃРєР°СЏ РєР»Р°СЃСЃРёРєР°",
                description: "РњРёРЅРёРјР°Р»РёСЃС‚РёС‡РЅС‹Рµ СЃРёР»СѓСЌС‚С‹ РґР»СЏ Р»СЋР±РѕРіРѕ СЃРµР·РѕРЅР°.",
                price: "РѕС‚ 6 500 в‚Ѕ",
                badge: Some("РќРѕРІРёРЅРєР°"),
            },
        ],
        _ => vec![
            ProductCardData {
                title: "Smart accessories",
                description: "Functional gadgets for everyday life.",
                price: "from $89",
                badge: Some("Best seller"),
            },
            ProductCardData {
                title: "Eco collection",
                description: "Natural materials with premium finishing.",
                price: "from $49",
                badge: None,
            },
            ProductCardData {
                title: "City essentials",
                description: "Minimal silhouettes for every season.",
                price: "from $129",
                badge: Some("New"),
            },
        ],
    }
}

#[component]
fn ProductCard(product: ProductCardData, badge_new: String, cta_view: String) -> impl IntoView {
    let badge = product.badge.map(|b| b.to_string()).unwrap_or(badge_new);
    view! {
        <div class="rounded-xl border border-border bg-card shadow">
            <div class="h-40 w-full rounded-t-xl bg-gradient-to-br from-primary/10 to-secondary" />
            <div class="p-5 space-y-3">
                <div class="flex items-start justify-between gap-2">
                    <h3 class="text-base font-semibold text-card-foreground">{product.title}</h3>
                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-medium text-secondary-foreground">{badge}</span>
                </div>
                <p class="text-sm text-muted-foreground">{product.description}</p>
                <div class="flex items-center justify-between pt-1">
                    <span class="text-lg font-semibold text-foreground">{product.price}</span>
                    <button class="inline-flex items-center justify-center rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground shadow hover:bg-primary/90 transition-colors">
                        {cta_view}
                    </button>
                </div>
            </div>
        </div>
    }
}

#[component]
fn StorefrontShellContent(locale: String) -> impl IntoView {
    let i18n = use_i18n();
    let products = featured_products(locale.as_str());
    let enabled_modules = use_enabled_modules().get();
    let module_sections =
        components_for_slot(StorefrontSlot::HomeAfterHero, Some(&enabled_modules));
    let badge_new = t_string!(i18n, badge.new).to_string();
    let cta_view = t_string!(i18n, cta.view).to_string();

    view! {
        <div class="min-h-screen bg-background text-foreground">
            <header class="sticky top-0 z-40 border-b border-border bg-background/95 backdrop-blur">
                <div class="container-app flex h-14 w-full items-center">
                    <div class="flex-1">
                        <a class="text-xl font-bold text-foreground hover:text-primary transition-colors" href="/">
                            "RusToK"
                        </a>
                    </div>
                    <nav class="hidden lg:flex items-center gap-6">
                        <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#home">{t_string!(i18n, nav.home)}</a>
                        <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#catalog">{t_string!(i18n, nav.catalog)}</a>
                        <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#about">{t_string!(i18n, nav.about)}</a>
                        <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#contact">{t_string!(i18n, nav.contact)}</a>
                    </nav>
                    <div class="flex items-center gap-3 ml-6">
                        <div class="relative">
                            <details class="group">
                                <summary class="inline-flex items-center gap-1 rounded-md border border-input bg-background px-3 py-1.5 text-sm text-foreground cursor-pointer hover:bg-accent hover:text-accent-foreground transition-colors list-none">
                                    {t_string!(i18n, nav.language)}
                                </summary>
                                <ul class="absolute right-0 mt-1 w-32 rounded-md border border-border bg-popover p-1 shadow-md z-50">
                                    <li>
                                        <a class="block rounded px-3 py-1.5 text-sm text-popover-foreground hover:bg-accent hover:text-accent-foreground transition-colors" href="/?lang=en">
                                            "English"
                                        </a>
                                    </li>
                                    <li>
                                        <a class="block rounded px-3 py-1.5 text-sm text-popover-foreground hover:bg-accent hover:text-accent-foreground transition-colors" href="/?lang=ru">
                                            "Р СѓСЃСЃРєРёР№"
                                        </a>
                                    </li>
                                </ul>
                            </details>
                        </div>
                        <a class="inline-flex items-center justify-center rounded-md bg-primary px-4 py-1.5 text-sm font-medium text-primary-foreground shadow hover:bg-primary/90 transition-colors" href="#catalog">
                            {t_string!(i18n, cta.primary)}
                        </a>
                    </div>
                </div>
            </header>

            <main class="space-y-20">
                <section id="home" class="bg-background">
                    <div class="container-app flex flex-col gap-10 py-16 lg:flex-row lg:items-center lg:justify-between">
                        <div class="max-w-xl space-y-6">
                            <h1 class="text-4xl font-bold text-foreground lg:text-5xl">
                                {t_string!(i18n, hero.title)}
                            </h1>
                            <p class="text-lg text-muted-foreground">{t_string!(i18n, hero.subtitle)}</p>
                            <div class="flex flex-wrap gap-3">
                                <a class="inline-flex items-center justify-center rounded-md bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground shadow hover:bg-primary/90 transition-colors" href="#catalog">
                                    {t_string!(i18n, cta.primary)}
                                </a>
                                <a class="inline-flex items-center justify-center rounded-md border border-input bg-background px-5 py-2.5 text-sm font-medium text-foreground shadow-sm hover:bg-accent hover:text-accent-foreground transition-colors" href="#about">
                                    {t_string!(i18n, cta.secondary)}
                                </a>
                            </div>
                        </div>
                        <div class="grid grid-cols-1 divide-y divide-border rounded-xl border border-border bg-card shadow sm:grid-cols-3 sm:divide-x sm:divide-y-0 lg:w-auto">
                            <div class="p-6 text-center">
                                <p class="text-sm text-muted-foreground">{t_string!(i18n, stat.ssrTitle)}</p>
                                <p class="mt-1 text-2xl font-bold text-foreground">{t_string!(i18n, stat.ssrValue)}</p>
                                <p class="mt-1 text-xs text-muted-foreground">{t_string!(i18n, stat.ssrDesc)}</p>
                            </div>
                            <div class="p-6 text-center">
                                <p class="text-sm text-muted-foreground">{t_string!(i18n, stat.i18nTitle)}</p>
                                <p class="mt-1 text-2xl font-bold text-foreground">{t_string!(i18n, stat.i18nValue)}</p>
                                <p class="mt-1 text-xs text-muted-foreground">{t_string!(i18n, stat.i18nDesc)}</p>
                            </div>
                            <div class="p-6 text-center">
                                <p class="text-sm text-muted-foreground">{t_string!(i18n, stat.modulesTitle)}</p>
                                <p class="mt-1 text-2xl font-bold text-foreground">{t_string!(i18n, stat.modulesValue)}</p>
                                <p class="mt-1 text-xs text-muted-foreground">{t_string!(i18n, stat.modulesDesc)}</p>
                            </div>
                        </div>
                    </div>
                </section>

                {module_sections
                    .into_iter()
                    .map(|module| (module.render)())
                    .collect_view()}

                <section id="catalog" class="container-app">
                    <div class="mx-auto max-w-6xl space-y-6">
                        <div>
                            <h2 class="text-3xl font-bold text-foreground">{t_string!(i18n, featured.title)}</h2>
                            <p class="mt-1 text-muted-foreground">{t_string!(i18n, featured.subtitle)}</p>
                        </div>
                        <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
                            {products
                                .into_iter()
                                .map(|product| {
                                    view! {
                                        <ProductCard
                                            product=product
                                            badge_new=badge_new.clone()
                                            cta_view=cta_view.clone()
                                        />
                                    }
                                })
                                .collect_view()}
                        </div>
                    </div>
                </section>

                <section id="about" class="container-app">
                    <div class="mx-auto grid max-w-6xl gap-6 lg:grid-cols-2">
                        <div class="rounded-xl border border-border bg-card p-6 shadow space-y-3">
                            <h3 class="text-lg font-semibold text-card-foreground">{t_string!(i18n, story.title)}</h3>
                            <p class="text-sm text-muted-foreground">{t_string!(i18n, story.body)}</p>
                            <div class="flex gap-2 flex-wrap">
                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-foreground">"GraphQL"</span>
                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-foreground">"Events"</span>
                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-foreground">"Search"</span>
                            </div>
                        </div>
                        <div class="rounded-xl border border-border bg-card p-6 shadow space-y-4">
                            <h3 class="text-lg font-semibold text-card-foreground">{t_string!(i18n, newsletter.title)}</h3>
                            <p class="text-sm text-muted-foreground">{t_string!(i18n, newsletter.body)}</p>
                            <div class="flex gap-2">
                                <input
                                    class="flex-1 rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                                    placeholder=t_string!(i18n, newsletter.placeholder)
                                />
                                <button class="inline-flex items-center justify-center rounded-md bg-secondary px-4 py-2 text-sm font-medium text-secondary-foreground shadow-sm hover:bg-secondary/80 transition-colors">
                                    {t_string!(i18n, newsletter.cta)}
                                </button>
                            </div>
                            <span class="text-xs text-muted-foreground">{t_string!(i18n, newsletter.note)}</span>
                        </div>
                    </div>
                </section>
            </main>

            <footer id="contact" class="mt-20 border-t border-border bg-muted/40 px-4 py-10">
                <div class="container-app space-y-3 text-center">
                    <p class="text-sm text-muted-foreground">{t_string!(i18n, footer.tagline)}</p>
                    <div class="flex justify-center gap-3">
                        <span class="inline-flex items-center rounded-full bg-primary px-2.5 py-0.5 text-xs font-medium text-primary-foreground">"SSR"</span>
                        <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-medium text-secondary-foreground">"Tailwind"</span>
                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-foreground">"shadcn"</span>
                    </div>
                </div>
            </footer>
        </div>
    }
}

#[component]
fn StorefrontShell(locale: String, enabled_modules: Vec<String>) -> impl IntoView {
    init_modules();

    view! {
        <EnabledModulesProvider initial_modules=enabled_modules>
            <StorefrontShellContent locale=locale />
        </EnabledModulesProvider>
    }
}

async fn render_shell(locale: &str) -> String {
    let locale_enum = match locale {
        "ru" => Locale::ru,
        _ => Locale::en,
    };
    let enabled_modules = match fetch_enabled_modules().await {
        Ok(modules) => modules,
        Err(err) => {
            eprintln!("failed to fetch enabled modules for storefront SSR: {err}");
            Vec::new()
        }
    };
    let app_html = leptos::prelude::Owner::new().with(|| {
        use leptos::prelude::Signal;
        leptos_i18n::context::provide_i18n_subcontext::<Locale>(Some(Signal::stored(locale_enum)));
        let locale = locale.to_string();
        view! { <StorefrontShell locale=locale enabled_modules=enabled_modules /> }.to_html()
    });
    format!(
        r#"<!DOCTYPE html>
<html lang="{locale}">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>RusToK Storefront</title>
  <link rel="stylesheet" href="/assets/app.css" />
</head>
<body>
  <div id="app">{app_html}</div>
</body>
</html>"#,
        locale = locale,
        app_html = app_html
    )
}

fn router() -> Router {
    Router::new()
        .route(
            "/",
            get(
                |axum::extract::Query(params): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move {
                    let locale = params
                        .get("lang")
                        .map(|value| value.to_lowercase())
                        .unwrap_or_else(|| "en".to_string());
                    render_shell(locale.as_str()).await
                },
            ),
        )
        .route("/assets/app.css", get(css_handler))
}

fn static_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static")
}

async fn css_handler() -> impl IntoResponse {
    let css_path = static_dir().join("app.css");
    match tokio::fs::read(css_path).await {
        Ok(contents) => ([(header::CONTENT_TYPE, "text/css")], contents).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = router();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3100").await?;
    println!("Storefront running on http://0.0.0.0:3100");
    axum::serve(listener, app).await?;
    Ok(())
}
