use leptos::prelude::*;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{StorefrontRegion, StorefrontRegionsData};
use crate::{core, transport};

#[component]
pub fn RegionView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_region_id =
        read_route_query_value(&route_context, core::SELECTED_REGION_QUERY_KEY);
    let route_state =
        core::region_route_state(route_context.route_segment.as_deref(), selected_region_id);
    let selected_region_id = route_state.selected_region_id.clone();
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

    let resource_locale = selected_locale.clone();
    let error_locale = selected_locale.clone();
    let resource = Resource::new_blocking(
        move || (selected_region_id.clone(), resource_locale.clone()),
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
                        let error_locale = error_locale.clone();
                        Suspend::new(async move {
                            match resource.await {
                                Ok(data) => view! { <RegionShowcase data /> }.into_any(),
                                Err(err) => {
                                    let error_view_model = core::region_error_view_model(
                                        (&err).into(),
                                        load_error,
                                        t(
                                            error_locale.as_deref(),
                                            "region.error.nativeUnavailable",
                                            "The native region data path is unavailable for this request.",
                                        ),
                                        t(
                                            error_locale.as_deref(),
                                            "region.error.fallbackUnavailable",
                                            "Both native and GraphQL region data paths are unavailable for this request.",
                                        ),
                                        t(
                                            error_locale.as_deref(),
                                            "region.error.status.nativeUnavailable",
                                            "Native unavailable",
                                        ),
                                        t(
                                            error_locale.as_deref(),
                                            "region.error.status.fallbackUnavailable",
                                            "Fallback unavailable",
                                        ),
                                        t(error_locale.as_deref(), "region.error.nativeLabel", "native"),
                                        t(error_locale.as_deref(), "region.error.graphqlLabel", "graphql"),
                                    );
                                    view! { <RegionErrorMessage error=error_view_model /> }.into_any()
                                },
                            }
                        })
                    }}
                </Suspense>
            </div>
        </section>
    }
}

#[component]
fn RegionErrorMessage(error: core::RegionErrorViewModel) -> impl IntoView {
    region_error_message_view(error)
}

fn region_error_message_view(error: core::RegionErrorViewModel) -> impl IntoView {
    let dom_evidence = core::region_error_dom_evidence(&error);

    view! {
        <div
            class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive"
            data-region-error-status=dom_evidence.status_value
            data-region-error-locale-key=dom_evidence.locale_key_value
        >
            <div class="flex flex-wrap items-center gap-2">
                <span class="rounded-full border border-destructive/30 px-2 py-0.5 font-mono text-[0.68rem] uppercase tracking-[0.16em]">{error.status_code.as_str()}</span>
                <span class="text-xs font-medium uppercase tracking-[0.16em]">{error.status_label}</span>
            </div>
            <p class="mt-2 font-medium">{error.title}</p>
            <p class="mt-1">{error.body}</p>
            {error.technical_detail.map(|detail| view! {
                <p class="mt-2 font-mono text-xs opacity-80">{detail}</p>
            })}
        </div>
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
    let view_model = core::selected_region_card_view_model(
        &region,
        tax_included_label.as_str(),
        tax_excluded_label.as_str(),
        &t(locale.as_deref(), "region.common.countries", "countries"),
        &t(
            locale.as_deref(),
            "region.selected.noCountries",
            "No countries configured",
        ),
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
    let country_policy_summaries = view_model.country_policy_summaries.clone();
    let has_country_policy_summaries = !country_policy_summaries.is_empty();

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                <span>{view_model.currency_code.clone()}</span>
                <span>"|"</span>
                <span>{view_model.tax_mode_label.clone()}</span>
                <span>"|"</span>
                <span>{view_model.countries_count_label.clone()}</span>
            </div>
            <h3 class="mt-4 text-3xl font-semibold text-foreground">{view_model.name.clone()}</h3>
            <p class="mt-4 text-sm leading-7 text-muted-foreground">
                {t(locale.as_deref(), "region.selected.body", "This region defines the storefront baseline for supported countries, currency, and tax semantics.")}
            </p>
            <div class="mt-6 grid gap-3 md:grid-cols-3">
                {view_model.metrics.clone().into_iter().map(|metric| view! {
                    <MetricCard title=metric.title value=metric.value />
                }).collect_view()}
            </div>
            <div class="mt-6 rounded-2xl border border-border bg-card p-5">
                <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(locale.as_deref(), "region.selected.countries", "Supported countries")}</h4>
                <p class="mt-3 text-sm text-muted-foreground">{view_model.countries_summary.clone()}</p>
            </div>
            <Show when=move || has_country_policy_summaries>
                <div class="mt-6 rounded-2xl border border-border bg-card p-5">
                    <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(locale.as_deref(), "region.selected.countryPolicies", "Country-specific tax policies")}</h4>
                    <div class="mt-3 space-y-2 text-sm text-muted-foreground">
                        {country_policy_summaries.iter().map(|summary| view! {
                            <p>{summary.clone()}</p>
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
    let route_state = core::region_route_state(route_context.route_segment.as_deref(), None);
    let module_route_base = route_context.module_route_base(route_state.route_segment.as_str());
    let view_model = core::region_rail_view_model(
        module_route_base.as_str(),
        &items,
        total,
        core::RegionRailLabels {
            title: t(locale.as_deref(), "region.list.title", "Available regions"),
            total_template: t(locale.as_deref(), "region.list.total", "{count} total"),
            empty_message: t(
                locale.as_deref(),
                "region.list.empty",
                "No regions are available for storefront discovery yet.",
            ),
            open_label: t(locale.as_deref(), "region.list.open", "Open"),
            tax_included_label: t(
                locale.as_deref(),
                "region.common.taxIncluded",
                "tax included",
            ),
            tax_excluded_label: t(
                locale.as_deref(),
                "region.common.taxExcluded",
                "tax excluded",
            ),
            empty_countries_label: t(
                locale.as_deref(),
                "region.common.noCountries",
                "no countries",
            ),
            tax_rate_label: t(locale.as_deref(), "region.common.taxRate", "tax rate"),
            tax_provider_label: t(
                locale.as_deref(),
                "region.common.taxProvider",
                "tax provider",
            ),
        },
    );

    if view_model.items.is_empty() {
        return view! { <article class="rounded-3xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{view_model.empty_message}</article> }.into_any();
    }

    let open_label = view_model.open_label.clone();

    view! {
        <div class="space-y-4">
            <div class="flex items-center justify-between gap-3">
                <h3 class="text-lg font-semibold text-card-foreground">{view_model.title.clone()}</h3>
                <span class="text-sm text-muted-foreground">
                    {view_model.total_label.clone()}
                </span>
            </div>
            <div class="space-y-3">
                {view_model.items.into_iter().map(|item| {
                    let open_label = open_label.clone();
                    view! {
                        <article class="rounded-2xl border border-border bg-background p-5">
                            <div class="flex items-start justify-between gap-3">
                                <div class="space-y-2">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <h4 class="text-base font-semibold text-card-foreground">{item.name.clone()}</h4>
                                        <span class="inline-flex rounded-full border border-border px-3 py-1 text-xs text-muted-foreground">
                                            {item.tax_mode_label.clone()}
                                        </span>
                                    </div>
                                    <p class="text-sm text-muted-foreground">{item.country_summary.clone()}</p>
                                    <p class="text-xs text-muted-foreground">{item.tax_summary.clone()}</p>
                                </div>
                                <a
                                    class="inline-flex text-sm font-medium text-primary hover:underline"
                                    href=item.href
                                    data-region-route-query-key=item.query_key
                                    data-region-route-query-value=item.query_value.clone().unwrap_or_default()
                                >
                                    {open_label}
                                </a>
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

#[cfg(all(test, feature = "ssr"))]
mod ssr_tests {
    use super::*;

    #[test]
    fn region_error_message_ssr_exposes_host_visible_dom_evidence() {
        let error = core::region_error_view_model(
            core::RegionErrorEvidence {
                failed_path: core::RegionStorefrontErrorPath::GraphqlFallback,
                fallback_attempted: true,
                native_error: Some("native failed".to_string()),
                graphql_error: Some("graphql failed".to_string()),
            },
            "Failed to load region storefront data".to_string(),
            "The native region data path is unavailable for this request.".to_string(),
            "Both native and GraphQL region data paths are unavailable for this request."
                .to_string(),
            "Native unavailable".to_string(),
            "Fallback unavailable".to_string(),
            "native".to_string(),
            "graphql".to_string(),
        );

        let html = region_error_message_view(error).into_view().to_html();

        assert!(
            html.contains(r#"data-region-error-status="fallback_unavailable""#),
            "rendered error message should expose stable status code: {html}"
        );
        assert!(
            html.contains(
                r#"data-region-error-locale-key="region.error.status.fallbackUnavailable""#
            ),
            "rendered error message should expose status locale key: {html}"
        );
    }

    #[test]
    fn region_rail_ssr_exposes_route_query_dom_evidence() {
        let region = StorefrontRegion {
            id: "eu".to_string(),
            name: "Europe".to_string(),
            currency_code: "EUR".to_string(),
            tax_provider_id: Some("default".to_string()),
            tax_rate: "20".to_string(),
            tax_included: true,
            country_tax_policies: vec![crate::model::StorefrontRegionCountryTaxPolicy {
                country_code: "DE".to_string(),
                tax_rate: "19".to_string(),
                tax_included: false,
            }],
            countries: vec!["DE".to_string(), "FR".to_string()],
        };

        let html = view! { <RegionRail items=vec![region] total=1 /> }.to_html();

        assert!(
            html.contains(r#"href="/modules/regions?region=eu""#),
            "rendered rail link should use core route/query href: {html}"
        );
        assert!(
            html.contains(r#"data-region-route-query-key="region""#),
            "rendered rail link should expose selected-region query key: {html}"
        );
        assert!(
            html.contains(r#"data-region-route-query-value="eu""#),
            "rendered rail link should expose selected-region query value: {html}"
        );
    }
}
