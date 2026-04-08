mod api;
mod i18n;
mod model;

use leptos::prelude::*;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{StorefrontRegion, StorefrontRegionsData};

#[component]
pub fn RegionView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_region_id = route_context.query_value("region").map(ToOwned::to_owned);
    let selected_locale = route_context.locale.clone();
    let badge = t(selected_locale.as_deref(), "region.badge", "region");
    let title = t(
        selected_locale.as_deref(),
        "region.title",
        "Storefront region discovery from the module package",
    );
    let subtitle = t(
        selected_locale.as_deref(),
        "region.subtitle",
        "This public route reads region, country, currency, and tax baseline data through the region-owned storefront surface.",
    );
    let load_error = t(
        selected_locale.as_deref(),
        "region.error.loadStorefront",
        "Failed to load region storefront data",
    );

    let resource = Resource::new_blocking(
        move || (selected_region_id.clone(), selected_locale.clone()),
        move |(selected_region_id, locale)| async move {
            api::fetch_storefront_regions(selected_region_id, locale).await
        },
    );

    view! {
        <section class="rounded-3xl border border-border bg-card p-8 shadow-sm">
            <div class="max-w-3xl space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">{badge}</span>
                <h2 class="text-3xl font-semibold text-card-foreground">{title}</h2>
                <p class="text-sm text-muted-foreground">{subtitle}</p>
            </div>

            <div class="mt-8">
                <Suspense fallback=|| view! { <div class="space-y-4"><div class="h-40 animate-pulse rounded-3xl bg-muted"></div><div class="grid gap-3 md:grid-cols-3"><div class="h-28 animate-pulse rounded-2xl bg-muted"></div><div class="h-28 animate-pulse rounded-2xl bg-muted"></div><div class="h-28 animate-pulse rounded-2xl bg-muted"></div></div></div> }>
                    {move || {
                        let resource = resource.clone();
                        let load_error = load_error.clone();
                        Suspend::new(async move {
                            match resource.await {
                                Ok(data) => view! { <RegionShowcase data /> }.into_any(),
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
fn RegionShowcase(data: StorefrontRegionsData) -> impl IntoView {
    let total = data.regions.len();
    let regions = data.regions;

    view! {
        <div class="grid gap-6 xl:grid-cols-[minmax(0,1.05fr)_minmax(0,0.95fr)]">
            <SelectedRegionCard region=data.selected_region />
            <RegionRail items=regions total=total />
        </div>
    }
}

#[component]
fn SelectedRegionCard(region: Option<StorefrontRegion>) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let Some(region) = region else {
        return view! {
            <article class="rounded-3xl border border-dashed border-border p-8">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {t(locale.as_deref(), "region.selected.emptyTitle", "No storefront regions available")}
                </h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {t(locale.as_deref(), "region.selected.emptyBody", "Create a region in the region admin package or enable region data for the current tenant first.")}
                </p>
            </article>
        }.into_any();
    };

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                <span>{region.currency_code.clone()}</span>
                <span>"|"</span>
                <span>{if region.tax_included { t(locale.as_deref(), "region.common.taxIncluded", "tax included") } else { t(locale.as_deref(), "region.common.taxExcluded", "tax excluded") }}</span>
                <span>"|"</span>
                <span>{format!("{} {}", region.countries.len(), t(locale.as_deref(), "region.common.countries", "countries"))}</span>
            </div>
            <h3 class="mt-4 text-3xl font-semibold text-foreground">{region.name.clone()}</h3>
            <p class="mt-4 text-sm leading-7 text-muted-foreground">
                {t(locale.as_deref(), "region.selected.body", "This region defines the storefront baseline for supported countries, currency, and tax semantics.")}
            </p>
            <div class="mt-6 grid gap-3 md:grid-cols-3">
                <MetricCard title=t(locale.as_deref(), "region.selected.currency", "Currency") value=region.currency_code.clone() />
                <MetricCard title=t(locale.as_deref(), "region.selected.taxRate", "Tax rate") value=region.tax_rate.clone() />
                <MetricCard title=t(locale.as_deref(), "region.selected.coverage", "Coverage") value=region.countries.len().to_string() />
            </div>
            <div class="mt-6 rounded-2xl border border-border bg-card p-5">
                <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(locale.as_deref(), "region.selected.countries", "Supported countries")}</h4>
                <p class="mt-3 text-sm text-muted-foreground">{region.countries.join(", ")}</p>
            </div>
        </article>
    }.into_any()
}

#[component]
fn RegionRail(items: Vec<StorefrontRegion>, total: usize) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let route_segment = route_context
        .route_segment
        .as_ref()
        .cloned()
        .unwrap_or_else(|| "regions".to_string());
    let module_route_base = route_context.module_route_base(route_segment.as_str());

    if items.is_empty() {
        return view! { <article class="rounded-3xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{t(locale.as_deref(), "region.list.empty", "No regions are available for storefront discovery yet.")}</article> }.into_any();
    }

    view! {
        <div class="space-y-4">
            <div class="flex items-center justify-between gap-3">
                <h3 class="text-lg font-semibold text-card-foreground">{t(locale.as_deref(), "region.list.title", "Available regions")}</h3>
                <span class="text-sm text-muted-foreground">
                    {t(locale.as_deref(), "region.list.total", "{count} total").replace("{count}", &total.to_string())}
                </span>
            </div>
            <div class="space-y-3">
                {items.into_iter().map(|region| {
                    let locale = locale.clone();
                    let href = format!("{module_route_base}?region={}", region.id);
                    view! {
                        <article class="rounded-2xl border border-border bg-background p-5">
                            <div class="flex items-start justify-between gap-3">
                                <div class="space-y-2">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <h4 class="text-base font-semibold text-card-foreground">{region.name.clone()}</h4>
                                        <span class="inline-flex rounded-full border border-border px-3 py-1 text-xs text-muted-foreground">
                                            {if region.tax_included { t(locale.as_deref(), "region.common.taxIncluded", "tax included") } else { t(locale.as_deref(), "region.common.taxExcluded", "tax excluded") }}
                                        </span>
                                    </div>
                                    <p class="text-sm text-muted-foreground">{format!("{} | {}", region.currency_code, region.countries.join(", "))}</p>
                                    <p class="text-xs text-muted-foreground">{format!("{} {}", region.tax_rate, t(locale.as_deref(), "region.common.taxRate", "tax rate"))}</p>
                                </div>
                                <a class="inline-flex text-sm font-medium text-primary hover:underline" href=href>{t(locale.as_deref(), "region.list.open", "Open")}</a>
                            </div>
                        </article>
                    }
                }).collect_view()}
            </div>
        </div>
    }.into_any()
}

#[component]
fn MetricCard(title: String, value: String) -> impl IntoView {
    view! { <article class="rounded-2xl border border-border bg-card p-4"><div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{title}</div><div class="mt-2 text-lg font-semibold text-card-foreground">{value}</div></article> }
}
