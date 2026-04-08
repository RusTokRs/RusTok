mod api;
mod i18n;
mod model;

use std::collections::BTreeSet;

use leptos::prelude::*;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{
    PricingPrice, PricingProductDetail, PricingProductListItem, PricingVariant,
    StorefrontPricingData,
};

#[component]
pub fn PricingView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_handle = route_context.query_value("handle").map(ToOwned::to_owned);
    let selected_locale = route_context.locale.clone();
    let badge = t(selected_locale.as_deref(), "pricing.badge", "pricing");
    let title = t(
        selected_locale.as_deref(),
        "pricing.title",
        "Public pricing atlas from the pricing module",
    );
    let subtitle = t(
        selected_locale.as_deref(),
        "pricing.subtitle",
        "This storefront route reads pricing visibility, currency coverage and sale markers through the pricing-owned package, while GraphQL stays available as fallback.",
    );
    let load_error = t(
        selected_locale.as_deref(),
        "pricing.error.load",
        "Failed to load storefront pricing data",
    );

    let resource = Resource::new_blocking(
        move || (selected_handle.clone(), selected_locale.clone()),
        move |(handle, locale)| async move { api::fetch_storefront_pricing(handle, locale).await },
    );

    view! {
        <section class="rounded-[2rem] border border-border bg-card p-8 shadow-sm">
            <div class="max-w-3xl space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">{badge}</span>
                <h2 class="text-3xl font-semibold text-card-foreground">{title}</h2>
                <p class="text-sm text-muted-foreground">{subtitle}</p>
            </div>
            <div class="mt-8">
                <Suspense fallback=|| view! { <div class="space-y-4"><div class="h-48 animate-pulse rounded-3xl bg-muted"></div><div class="grid gap-3 md:grid-cols-3"><div class="h-28 animate-pulse rounded-2xl bg-muted"></div><div class="h-28 animate-pulse rounded-2xl bg-muted"></div><div class="h-28 animate-pulse rounded-2xl bg-muted"></div></div></div> }>
                    {move || {
                        let resource = resource.clone();
                        let load_error = load_error.clone();
                        Suspend::new(async move {
                            match resource.await {
                                Ok(data) => view! { <PricingShowcase data /> }.into_any(),
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
fn PricingShowcase(data: StorefrontPricingData) -> impl IntoView {
    view! {
        <div class="grid gap-6 xl:grid-cols-[minmax(0,1.08fr)_minmax(0,0.92fr)]">
            <SelectedPricingCard product=data.selected_product />
            <PricingRail items=data.products.items total=data.products.total />
        </div>
    }
}

#[component]
fn SelectedPricingCard(product: Option<PricingProductDetail>) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let Some(product) = product else {
        return view! {
            <article class="rounded-3xl border border-dashed border-border p-8">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {t(locale.as_deref(), "pricing.selected.emptyTitle", "No pricing card selected")}
                </h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {t(locale.as_deref(), "pricing.selected.emptyBody", "Open a published product through `?handle=` to inspect variant-level pricing coverage and sale markers.")}
                </p>
            </article>
        }
        .into_any();
    };

    let translation = product.translations.first().cloned();
    let title = translation
        .as_ref()
        .map(|item| item.title.clone())
        .unwrap_or_else(|| {
            t(
                locale.as_deref(),
                "pricing.selected.untitled",
                "Untitled product",
            )
        });
    let description = translation
        .and_then(|item| item.description)
        .unwrap_or_else(|| {
            t(
                locale.as_deref(),
                "pricing.selected.noDescription",
                "No localized merchandising copy yet.",
            )
        });
    let summary = summarize_pricing(product.variants.as_slice());

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                <span>{product.product_type.unwrap_or_else(|| t(locale.as_deref(), "pricing.selected.catalog", "catalog"))}</span>
                <span>"|"</span>
                <span>{product.vendor.unwrap_or_else(|| t(locale.as_deref(), "pricing.selected.vendorFallback", "independent label"))}</span>
                <span>"|"</span>
                <span>{product.published_at.unwrap_or_else(|| t(locale.as_deref(), "pricing.selected.unscheduled", "scheduled later"))}</span>
            </div>
            <h3 class="mt-4 text-3xl font-semibold text-foreground">{title}</h3>
            <p class="mt-4 text-sm leading-7 text-muted-foreground">{description}</p>
            <div class="mt-6 grid gap-3 md:grid-cols-3">
                <MetricCard title=t(locale.as_deref(), "pricing.selected.currencies", "Currencies") value=summary.currency_count.to_string() />
                <MetricCard title=t(locale.as_deref(), "pricing.selected.saleVariants", "Sale variants") value=summary.sale_variant_count.to_string() />
                <MetricCard title=t(locale.as_deref(), "pricing.selected.variants", "Variants") value=summary.variant_count.to_string() />
            </div>
            <div class="mt-6 space-y-3">
                {product.variants.into_iter().map(|variant| {
                    let locale = locale.clone();
                    view! {
                        <article class="rounded-2xl border border-border bg-card p-5">
                            <div class="flex items-start justify-between gap-3">
                                <div class="space-y-2">
                                    <h4 class="text-base font-semibold text-card-foreground">{variant.title.clone()}</h4>
                                    <p class="text-xs text-muted-foreground">{format_variant_identity(locale.as_deref(), &variant)}</p>
                                    <p class="text-sm text-muted-foreground">{format_variant_prices(locale.as_deref(), variant.prices.as_slice())}</p>
                                </div>
                                <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", pricing_health_badge(&variant))>
                                    {pricing_health_label(locale.as_deref(), &variant)}
                                </span>
                            </div>
                        </article>
                    }
                }).collect_view()}
            </div>
        </article>
    }
    .into_any()
}

#[component]
fn PricingRail(items: Vec<PricingProductListItem>, total: u64) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let route_segment = route_context
        .route_segment
        .as_ref()
        .cloned()
        .unwrap_or_else(|| "pricing".to_string());
    let module_route_base = route_context.module_route_base(route_segment.as_str());

    if items.is_empty() {
        return view! { <article class="rounded-3xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{t(locale.as_deref(), "pricing.list.empty", "No published products with visible pricing are available yet.")}</article> }.into_any();
    }

    view! {
        <div class="space-y-4">
            <div class="flex items-center justify-between gap-3">
                <h3 class="text-lg font-semibold text-card-foreground">{t(locale.as_deref(), "pricing.list.title", "Pricing feed")}</h3>
                <span class="text-sm text-muted-foreground">
                    {t(locale.as_deref(), "pricing.list.total", "{count} total").replace("{count}", &total.to_string())}
                </span>
            </div>
            <div class="space-y-3">
                {items.into_iter().map(|product| {
                    let locale = locale.clone();
                    let href = format!("{module_route_base}?handle={}", product.handle);
                    let currencies = if product.currencies.is_empty() {
                        t(locale.as_deref(), "pricing.common.noCurrencies", "no currencies")
                    } else {
                        product.currencies.join(", ")
                    };
                    view! {
                        <article class="rounded-2xl border border-border bg-background p-5">
                            <div class="space-y-2">
                                <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{product.product_type.unwrap_or_else(|| t(locale.as_deref(), "pricing.selected.catalog", "catalog"))}</div>
                                <h4 class="text-base font-semibold text-card-foreground">{product.title}</h4>
                                <p class="text-sm text-muted-foreground">{product.vendor.unwrap_or_else(|| t(locale.as_deref(), "pricing.list.vendorFallback", "Independent label"))}</p>
                                <p class="text-xs text-muted-foreground">{currencies}</p>
                                <div class="grid gap-2 text-xs text-muted-foreground md:grid-cols-3">
                                    <span>{t(locale.as_deref(), "pricing.list.variants", "{count} variants").replace("{count}", &product.variant_count.to_string())}</span>
                                    <span>{t(locale.as_deref(), "pricing.list.sales", "{count} on sale").replace("{count}", &product.sale_variant_count.to_string())}</span>
                                    <span>{product.published_at.unwrap_or(product.created_at)}</span>
                                </div>
                            </div>
                            <div class="mt-4 flex justify-end">
                                <a class="inline-flex text-sm font-medium text-primary hover:underline" href=href>{t(locale.as_deref(), "pricing.list.open", "Open")}</a>
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

struct PricingSummary {
    currency_count: usize,
    sale_variant_count: usize,
    variant_count: usize,
}

fn summarize_pricing(variants: &[PricingVariant]) -> PricingSummary {
    let mut currencies = BTreeSet::new();
    let sale_variant_count = variants
        .iter()
        .filter(|variant| {
            variant.prices.iter().any(|price| {
                currencies.insert(price.currency_code.clone());
                price.on_sale
            })
        })
        .count();

    for variant in variants {
        for price in &variant.prices {
            currencies.insert(price.currency_code.clone());
        }
    }

    PricingSummary {
        currency_count: currencies.len(),
        sale_variant_count,
        variant_count: variants.len(),
    }
}

fn format_variant_identity(locale: Option<&str>, variant: &PricingVariant) -> String {
    if let Some(sku) = variant
        .sku
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        format!(
            "{}: {}",
            t(locale, "pricing.variant.sku", "SKU"),
            sku.trim()
        )
    } else {
        t(locale, "pricing.variant.noSku", "SKU not assigned")
    }
}

fn format_variant_prices(locale: Option<&str>, prices: &[PricingPrice]) -> String {
    if prices.is_empty() {
        return t(locale, "pricing.variant.noPrices", "No prices assigned");
    }

    prices
        .iter()
        .map(|price| {
            if let Some(compare) = price.compare_at_amount.as_deref() {
                format!(
                    "{} {} ({})",
                    price.currency_code,
                    price.amount,
                    t(locale, "pricing.variant.compareAt", "compare-at {value}")
                        .replace("{value}", compare),
                )
            } else {
                format!("{} {}", price.currency_code, price.amount)
            }
        })
        .collect::<Vec<_>>()
        .join(" • ")
}

fn pricing_health_label(locale: Option<&str>, variant: &PricingVariant) -> String {
    if variant.prices.is_empty() {
        return t(locale, "pricing.health.missing", "missing");
    }
    if variant.prices.iter().any(|price| price.on_sale) {
        return t(locale, "pricing.health.sale", "sale");
    }
    t(locale, "pricing.health.covered", "covered")
}

fn pricing_health_badge(variant: &PricingVariant) -> &'static str {
    if variant.prices.is_empty() {
        "border-destructive/30 text-destructive"
    } else if variant.prices.iter().any(|price| price.on_sale) {
        "border-emerald-500/30 text-emerald-700"
    } else {
        "border-border text-muted-foreground"
    }
}
