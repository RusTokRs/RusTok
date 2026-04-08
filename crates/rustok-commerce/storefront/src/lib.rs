mod api;
mod i18n;
mod model;

use leptos::prelude::*;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::StorefrontCommerceData;

#[component]
pub fn CommerceView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_locale = route_context.locale.clone();
    let badge = t(selected_locale.as_deref(), "commerce.badge", "commerce");
    let title = t(
        selected_locale.as_deref(),
        "commerce.title",
        "Commerce orchestration hub",
    );
    let subtitle = t(
        selected_locale.as_deref(),
        "commerce.subtitle",
        "Catalog, pricing, and regions now live in module-owned storefront packages. Commerce remains the aggregate storefront handoff for checkout context and cross-domain flow.",
    );
    let load_error = t(
        selected_locale.as_deref(),
        "commerce.error.load",
        "Failed to load commerce storefront data",
    );

    let resource = Resource::new_blocking(
        move || selected_locale.clone(),
        move |locale| async move { api::fetch_storefront_commerce(locale).await },
    );

    view! {
        <section class="rounded-[2rem] border border-border bg-card p-8 shadow-sm">
            <div class="max-w-3xl space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">{badge}</span>
                <h2 class="text-3xl font-semibold text-card-foreground">{title}</h2>
                <p class="text-sm text-muted-foreground">{subtitle}</p>
            </div>
            <div class="mt-8">
                <Suspense fallback=|| view! { <div class="space-y-4"><div class="h-48 animate-pulse rounded-3xl bg-muted"></div><div class="grid gap-3 md:grid-cols-2"><div class="h-48 animate-pulse rounded-2xl bg-muted"></div><div class="h-48 animate-pulse rounded-2xl bg-muted"></div></div></div> }>
                    {move || {
                        let resource = resource.clone();
                        let load_error = load_error.clone();
                        Suspend::new(async move {
                            match resource.await {
                                Ok(data) => view! { <CommerceShowcase data /> }.into_any(),
                                Err(err) => view! { <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("{}: {err}", load_error)}</div> }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </div>
        </section>
    }
}

#[component]
fn CommerceShowcase(data: StorefrontCommerceData) -> impl IntoView {
    view! {
        <div class="grid gap-6 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
            <ContextCard data=data />
            <SurfaceRail />
        </div>
    }
}

#[component]
fn ContextCard(data: StorefrontCommerceData) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let tenant_value = data.tenant_slug.unwrap_or_else(|| {
        t(
            locale.as_deref(),
            "commerce.context.tenantMissing",
            "host tenant",
        )
    });
    let channel_value = data
        .channel_slug
        .unwrap_or_else(|| t(locale.as_deref(), "commerce.context.empty", "not resolved"));
    let resolution_value = data
        .channel_resolution_source
        .unwrap_or_else(|| t(locale.as_deref(), "commerce.context.empty", "not resolved"));

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                    {t(locale.as_deref(), "commerce.context.badge", "storefront context")}
                </span>
                <h3 class="text-2xl font-semibold text-card-foreground">
                    {t(locale.as_deref(), "commerce.context.title", "Active storefront context")}
                </h3>
                <p class="text-sm leading-7 text-muted-foreground">
                    {t(locale.as_deref(), "commerce.context.subtitle", "This aggregate route now exposes only the request context that still coordinates cart, delivery selection, payment collection, and checkout orchestration.")}
                </p>
            </div>
            <div class="mt-6 grid gap-3 md:grid-cols-2">
                <MetricCard title=t(locale.as_deref(), "commerce.context.locale", "Effective locale") value=data.effective_locale />
                <MetricCard title=t(locale.as_deref(), "commerce.context.tenant", "Tenant") value=tenant_value />
                <MetricCard title=t(locale.as_deref(), "commerce.context.tenantDefault", "Tenant default locale") value=data.tenant_default_locale />
                <MetricCard title=t(locale.as_deref(), "commerce.context.channel", "Channel") value=channel_value />
            </div>
            <div class="mt-3">
                <MetricCard title=t(locale.as_deref(), "commerce.context.resolution", "Channel source") value=resolution_value />
            </div>
        </article>
    }
}

#[component]
fn SurfaceRail() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let region_href = route_context.module_route_base("regions");
    let product_href = route_context.module_route_base("products");
    let pricing_href = route_context.module_route_base("pricing");

    view! {
        <div class="space-y-4">
            <div class="space-y-2">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {t(locale.as_deref(), "commerce.surface.title", "Module-owned storefront surfaces")}
                </h3>
                <p class="text-sm text-muted-foreground">
                    {t(locale.as_deref(), "commerce.surface.subtitle", "Discovery now belongs to split modules. Commerce stays here only where flows still cross cart, region, pricing, order, and fulfillment boundaries.")}
                </p>
            </div>
            <div class="grid gap-3">
                <SurfaceCard
                    badge=t(locale.as_deref(), "commerce.surface.region.badge", "region")
                    title=t(locale.as_deref(), "commerce.surface.region.title", "Regions")
                    body=t(locale.as_deref(), "commerce.surface.region.body", "Region discovery now lives in the region-owned storefront package and owns country/currency selection.")
                    href=Some(region_href)
                />
                <SurfaceCard
                    badge=t(locale.as_deref(), "commerce.surface.product.badge", "product")
                    title=t(locale.as_deref(), "commerce.surface.product.title", "Catalog")
                    body=t(locale.as_deref(), "commerce.surface.product.body", "Published catalog discovery and product detail now live in the product-owned storefront package.")
                    href=Some(product_href)
                />
                <SurfaceCard
                    badge=t(locale.as_deref(), "commerce.surface.pricing.badge", "pricing")
                    title=t(locale.as_deref(), "commerce.surface.pricing.title", "Pricing")
                    body=t(locale.as_deref(), "commerce.surface.pricing.body", "Public pricing atlas, currency coverage, and sale markers now live in the pricing-owned storefront package.")
                    href=Some(pricing_href)
                />
                <SurfaceCard
                    badge=t(locale.as_deref(), "commerce.surface.aggregate.badge", "aggregate")
                    title=t(locale.as_deref(), "commerce.surface.aggregate.title", "Remaining aggregate scope")
                    body=t(locale.as_deref(), "commerce.surface.aggregate.body", "Cart context, shipping selection, payment collection, and checkout orchestration remain in commerce because they still coordinate multiple split modules.")
                    href=None
                />
            </div>
        </div>
    }
}

#[component]
fn SurfaceCard(badge: String, title: String, body: String, href: Option<String>) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;

    view! {
        <article class="rounded-2xl border border-border bg-background p-5">
            <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{badge}</div>
            <h4 class="mt-2 text-base font-semibold text-card-foreground">{title}</h4>
            <p class="mt-2 text-sm leading-7 text-muted-foreground">{body}</p>
            {match href {
                Some(href) => view! {
                    <div class="mt-4">
                        <a class="inline-flex text-sm font-medium text-primary hover:underline" href=href>
                            {t(locale.as_deref(), "commerce.surface.open", "Open")}
                        </a>
                    </div>
                }.into_any(),
                None => view! {
                    <div class="mt-4 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                        {t(locale.as_deref(), "commerce.surface.here", "stays here")}
                    </div>
                }.into_any(),
            }}
        </article>
    }
}

#[component]
fn MetricCard(title: String, value: String) -> impl IntoView {
    view! { <article class="rounded-2xl border border-border bg-card p-4"><div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{title}</div><div class="mt-2 text-lg font-semibold text-card-foreground">{value}</div></article> }
}
