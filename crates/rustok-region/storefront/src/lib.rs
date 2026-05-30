mod api;
mod core;
mod i18n;
mod model;
mod transport;

use leptos::prelude::*;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{StorefrontRegion, StorefrontRegionsData};

#[component]
pub fn RegionView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_region_id = read_route_query_value(&route_context, "region");
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
            transport::fetch_regions(selected_region_id, locale).await
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
                        let resource = resource;
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

    let country_tax_policies = region.country_tax_policies.clone();
    let country_tax_policies_for_show = country_tax_policies.clone();
    let country_tax_policies_for_list = country_tax_policies.clone();
    let tax_included_label = t(
        locale.as_deref(),
        "region.common.taxIncluded",
        "tax included",
    );
    let tax_excluded_label = t(
        locale.as_deref(),
        "region.common.taxExcluded",
        "tax excluded",
    );
    let tax_mode_label = core::tax_mode_label(
        region.tax_included,
        tax_included_label.as_str(),
        tax_excluded_label.as_str(),
    );
    let countries_count_label = core::country_count_label(
        region.countries.len(),
        &t(locale.as_deref(), "region.common.countries", "countries"),
    );
    let metrics = core::selected_region_metrics(
        &region,
        t(locale.as_deref(), "region.selected.currency", "Currency"),
        t(locale.as_deref(), "region.selected.taxRate", "Tax rate"),
        t(
            locale.as_deref(),
            "region.selected.taxProvider",
            "Tax provider",
        ),
        t(locale.as_deref(), "region.selected.coverage", "Coverage"),
        t(
            locale.as_deref(),
            "region.selected.countryPolicyCount",
            "Country policies",
        ),
    );
    let countries_summary = core::countries_summary(
        &region.countries,
        &t(
            locale.as_deref(),
            "region.selected.noCountries",
            "No countries configured",
        ),
    );

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                <span>{region.currency_code.clone()}</span>
                <span>"|"</span>
                <span>{tax_mode_label}</span>
                <span>"|"</span>
                <span>{countries_count_label}</span>
            </div>
            <h3 class="mt-4 text-3xl font-semibold text-foreground">{region.name.clone()}</h3>
            <p class="mt-4 text-sm leading-7 text-muted-foreground">
                {t(locale.as_deref(), "region.selected.body", "This region defines the storefront baseline for supported countries, currency, and tax semantics.")}
            </p>
            <div class="mt-6 grid gap-3 md:grid-cols-3">
                {metrics.into_iter().map(|metric| view! {
                    <MetricCard title=metric.title value=metric.value />
                }).collect_view()}
            </div>
            <div class="mt-6 rounded-2xl border border-border bg-card p-5">
                <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(locale.as_deref(), "region.selected.countries", "Supported countries")}</h4>
                <p class="mt-3 text-sm text-muted-foreground">{countries_summary}</p>
            </div>
            <Show when=move || !country_tax_policies_for_show.is_empty()>
                <div class="mt-6 rounded-2xl border border-border bg-card p-5">
                    <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(locale.as_deref(), "region.selected.countryPolicies", "Country-specific tax policies")}</h4>
                    <div class="mt-3 space-y-2 text-sm text-muted-foreground">
                        {country_tax_policies_for_list.iter().map(|policy| view! {
                            <p>{core::country_policy_summary(
                                policy,
                                tax_included_label.as_str(),
                                tax_excluded_label.as_str(),
                            )}</p>
                        }).collect_view()}
                    </div>
                </div>
            </Show>
        </article>
    }.into_any()
}

#[component]
fn RegionRail(items: Vec<StorefrontRegion>, total: usize) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let route_segment = core::route_segment_or_default(route_context.route_segment.as_deref());
    let module_route_base = route_context.module_route_base(route_segment.as_str());

    if items.is_empty() {
        return view! { <article class="rounded-3xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{t(locale.as_deref(), "region.list.empty", "No regions are available for storefront discovery yet.")}</article> }.into_any();
    }

    view! {
        <div class="space-y-4">
            <div class="flex items-center justify-between gap-3">
                <h3 class="text-lg font-semibold text-card-foreground">{t(locale.as_deref(), "region.list.title", "Available regions")}</h3>
                <span class="text-sm text-muted-foreground">
                    {core::count_label(&t(locale.as_deref(), "region.list.total", "{count} total"), total)}
                </span>
            </div>
            <div class="space-y-3">
                {items.into_iter().map(|region| {
                    let locale = locale.clone();
                    let view_model = core::rail_item_view_model(
                        module_route_base.as_str(),
                        &region,
                        &t(locale.as_deref(), "region.common.taxIncluded", "tax included"),
                        &t(locale.as_deref(), "region.common.taxExcluded", "tax excluded"),
                        &t(locale.as_deref(), "region.common.noCountries", "no countries"),
                        &t(locale.as_deref(), "region.common.taxRate", "tax rate"),
                        &t(locale.as_deref(), "region.common.taxProvider", "tax provider"),
                    );
                    view! {
                        <article class="rounded-2xl border border-border bg-background p-5">
                            <div class="flex items-start justify-between gap-3">
                                <div class="space-y-2">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <h4 class="text-base font-semibold text-card-foreground">{region.name.clone()}</h4>
                                        <span class="inline-flex rounded-full border border-border px-3 py-1 text-xs text-muted-foreground">
                                            {view_model.tax_mode_label.clone()}
                                        </span>
                                    </div>
                                    <p class="text-sm text-muted-foreground">{view_model.country_summary.clone()}</p>
                                    <p class="text-xs text-muted-foreground">{view_model.tax_summary.clone()}</p>
                                </div>
                                <a class="inline-flex text-sm font-medium text-primary hover:underline" href=view_model.href>{t(locale.as_deref(), "region.list.open", "Open")}</a>
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
