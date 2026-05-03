mod api;
mod i18n;
mod model;

use std::collections::BTreeSet;

use leptos::prelude::*;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;
use rustok_core::locale_tags_match;

use crate::i18n::t;
use crate::model::{
    PricingChannelOption, PricingPrice, PricingPriceListOption, PricingProductDetail,
    PricingProductListItem, PricingResolutionContext, PricingVariant, StorefrontPricingData,
};

#[component]
pub fn PricingView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_handle = read_route_query_value(&route_context, "handle");
    let selected_locale = route_context.locale.clone();
    let selected_currency_code = read_route_query_value(&route_context, "currency");
    let selected_region_id = read_route_query_value(&route_context, "region_id");
    let selected_price_list_id = read_route_query_value(&route_context, "price_list_id");
    let selected_channel_id = read_route_query_value(&route_context, "channel_id");
    let selected_channel_slug = read_route_query_value(&route_context, "channel_slug");
    let selected_quantity = read_route_query_value(&route_context, "quantity")
        .and_then(|value| value.parse::<i32>().ok());
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
        move || {
            (
                selected_handle.clone(),
                selected_locale.clone(),
                selected_currency_code.clone(),
                selected_region_id.clone(),
                selected_price_list_id.clone(),
                selected_channel_id.clone(),
                selected_channel_slug.clone(),
                selected_quantity,
            )
        },
        move |(
            handle,
            locale,
            currency_code,
            region_id,
            price_list_id,
            channel_id,
            channel_slug,
            quantity,
        )| async move {
            api::fetch_storefront_pricing(api::StorefrontPricingQuery {
                selected_handle: handle,
                locale,
                currency_code,
                region_id,
                price_list_id,
                channel_id,
                channel_slug,
                quantity,
            })
            .await
        },
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
            <SelectedPricingCard
                product=data.selected_product
                selected_handle=data.selected_handle
                resolution_context=data.resolution_context
                available_channels=data.available_channels
                active_price_lists=data.active_price_lists
            />
            <PricingRail items=data.products.items total=data.products.total />
        </div>
    }
}

#[component]
fn SelectedPricingCard(
    product: Option<PricingProductDetail>,
    selected_handle: Option<String>,
    resolution_context: Option<PricingResolutionContext>,
    available_channels: Vec<PricingChannelOption>,
    active_price_lists: Vec<PricingPriceListOption>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
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
    let route_segment = route_context
        .route_segment
        .as_ref()
        .cloned()
        .unwrap_or_else(|| "pricing".to_string());
    let module_route_base = route_context.module_route_base(route_segment.as_str());
    let product_module_route_base = route_context.module_route_base("products");

    let translation =
        pricing_translation_for_locale(product.translations.as_slice(), locale.as_deref()).cloned();
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
        .as_ref()
        .and_then(|item| item.description.clone())
        .unwrap_or_else(|| {
            t(
                locale.as_deref(),
                "pricing.selected.noDescription",
                "No localized merchandising copy yet.",
            )
        });
    let summary = summarize_pricing(product.variants.as_slice());
    let seller_boundary = format_seller_boundary(locale.as_deref(), product.seller_id.as_deref());
    let product_href = build_product_storefront_href(
        product_module_route_base.as_str(),
        selected_handle
            .as_deref()
            .or_else(|| translation.as_ref().map(|item| item.handle.as_str())),
    );

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                <span>{product.product_type.unwrap_or_else(|| t(locale.as_deref(), "pricing.selected.catalog", "catalog"))}</span>
                <span>"|"</span>
                <span>{product.vendor.unwrap_or_else(|| t(locale.as_deref(), "pricing.selected.vendorFallback", "independent label"))}</span>
                <span>"|"</span>
                <span>{product.published_at.unwrap_or_else(|| t(locale.as_deref(), "pricing.selected.unscheduled", "scheduled later"))}</span>
            </div>
            <p class="mt-3 text-xs font-medium text-muted-foreground">{seller_boundary}</p>
            {resolution_context.as_ref().map(|context| view! {
                <div class="mt-4 inline-flex flex-wrap items-center gap-2 rounded-2xl border border-primary/20 bg-primary/5 px-4 py-2 text-xs text-primary">
                    <span class="font-semibold uppercase tracking-[0.16em]">
                        {t(locale.as_deref(), "pricing.selected.effectiveContext", "effective context")}
                    </span>
                    <span>{format_effective_context(locale.as_deref(), context, active_price_lists.as_slice())}</span>
                </div>
            })}
            {resolution_context.as_ref().map(|context| view! {
                <ResolutionSelector
                    module_route_base=module_route_base.clone()
                    selected_handle=selected_handle.clone()
                    resolution_context=context.clone()
                    available_channels=available_channels.clone()
                    active_price_lists=active_price_lists.clone()
                />
            })}
            <h3 class="mt-4 text-3xl font-semibold text-foreground">{title}</h3>
            <p class="mt-4 text-sm leading-7 text-muted-foreground">{description}</p>
            <div class="mt-4">
                <a
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent"
                    href=product_href
                >
                    {t(
                        locale.as_deref(),
                        "pricing.selected.openProduct",
                        "Open product module",
                    )}
                </a>
            </div>
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
                                    {variant.effective_price.as_ref().map(|price| view! {
                                        <p class="text-sm font-medium text-foreground">
                                            {format_effective_price(locale.as_deref(), price)}
                                        </p>
                                    })}
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
fn ResolutionSelector(
    module_route_base: String,
    selected_handle: Option<String>,
    resolution_context: PricingResolutionContext,
    available_channels: Vec<PricingChannelOption>,
    active_price_lists: Vec<PricingPriceListOption>,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let current_price_list_id = resolution_context.price_list_id.clone();
    let current_channel_id = resolution_context.channel_id.clone();
    let current_channel_slug = resolution_context.channel_slug.clone();
    let base_params = PricingRouteParams {
        selected_handle: selected_handle.as_deref(),
        currency_code: Some(resolution_context.currency_code.as_str()),
        region_id: resolution_context.region_id.as_deref(),
        quantity: Some(resolution_context.quantity),
        ..PricingRouteParams::default()
    };
    let base_price_list_href = build_pricing_route_href(module_route_base.as_str(), base_params);
    let global_channel_href = build_pricing_route_href(
        module_route_base.as_str(),
        PricingRouteParams {
            price_list_id: resolution_context.price_list_id.as_deref(),
            ..base_params
        },
    );

    view! {
        <div class="mt-4 rounded-2xl border border-border bg-card p-4">
            <div class="space-y-1">
                <h4 class="text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground">
                    {t(locale.as_deref(), "pricing.selected.priceListSwitcherTitle", "price list")}
                </h4>
                <p class="text-sm text-muted-foreground">
                    {t(locale.as_deref(), "pricing.selected.priceListSwitcherSubtitle", "Switch between base prices and currently active price lists without leaving the pricing module route.")}
                </p>
            </div>
            <div class="mt-3 flex flex-wrap gap-2">
                <a
                    class={
                        let current_price_list_id = current_price_list_id.clone();
                        move || selector_badge_class(current_price_list_id.is_none())
                    }
                    href=base_price_list_href
                >
                    {t(locale.as_deref(), "pricing.selected.basePriceListFallback", "base prices")}
                </a>
                {active_price_lists.into_iter().map(|option| {
                    let href = build_pricing_route_href(module_route_base.as_str(), PricingRouteParams {
                        price_list_id: Some(option.id.as_str()),
                        channel_id: resolution_context.channel_id.as_deref(),
                        channel_slug: resolution_context.channel_slug.as_deref(),
                        ..base_params
                    });
                    let is_active = current_price_list_id.as_deref() == Some(option.id.as_str());
                    let label = format_price_list_option_label(locale.as_deref(), &option);
                    view! {
                        <a class=selector_badge_class(is_active) href=href>{label}</a>
                    }
                }).collect_view()}
            </div>
            <div class="mt-4 space-y-1">
                <h4 class="text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground">
                    {t(locale.as_deref(), "pricing.selected.channelSwitcherTitle", "channel")}
                </h4>
                <p class="text-sm text-muted-foreground">
                    {t(locale.as_deref(), "pricing.selected.channelSwitcherSubtitle", "Switch between global pricing and channel-scoped pricing without leaving the pricing module route.")}
                </p>
            </div>
            <div class="mt-3 flex flex-wrap gap-2">
                <a
                    class={
                        let current_channel_id = current_channel_id.clone();
                        let current_channel_slug = current_channel_slug.clone();
                        move || selector_badge_class(
                            current_channel_id.as_deref().map(str::trim).unwrap_or_default().is_empty()
                                && current_channel_slug.as_deref().map(str::trim).unwrap_or_default().is_empty()
                        )
                    }
                    href=global_channel_href
                >
                    {t(locale.as_deref(), "pricing.selected.globalChannelFallback", "global channel")}
                </a>
                {available_channels.into_iter().map(|option| {
                    let href = build_pricing_route_href(module_route_base.as_str(), PricingRouteParams {
                        price_list_id: resolution_context.price_list_id.as_deref(),
                        channel_id: Some(option.id.as_str()),
                        channel_slug: Some(option.slug.as_str()),
                        ..base_params
                    });
                    let is_active =
                        current_channel_id.as_deref() == Some(option.id.as_str())
                            || current_channel_slug.as_deref() == Some(option.slug.as_str());
                    let label = format_channel_option_label(locale.as_deref(), &option);
                    view! {
                        <a class=selector_badge_class(is_active) href=href>{label}</a>
                    }
                }).collect_view()}
            </div>
        </div>
    }
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
    let selected_currency_code = read_route_query_value(&route_context, "currency");
    let selected_region_id = read_route_query_value(&route_context, "region_id");
    let selected_price_list_id = read_route_query_value(&route_context, "price_list_id");
    let selected_channel_id = read_route_query_value(&route_context, "channel_id");
    let selected_channel_slug = read_route_query_value(&route_context, "channel_slug");
    let selected_quantity = read_route_query_value(&route_context, "quantity")
        .and_then(|value| value.parse::<i32>().ok());

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
                    let href = build_pricing_route_href(module_route_base.as_str(), PricingRouteParams {
                        selected_handle: Some(product.handle.as_str()),
                        currency_code: selected_currency_code.as_deref(),
                        region_id: selected_region_id.as_deref(),
                        price_list_id: selected_price_list_id.as_deref(),
                        channel_id: selected_channel_id.as_deref(),
                        channel_slug: selected_channel_slug.as_deref(),
                        quantity: selected_quantity,
                    });
                    let currencies = if product.currencies.is_empty() {
                        t(locale.as_deref(), "pricing.common.noCurrencies", "no currencies")
                    } else {
                        product.currencies.join(", ")
                    };
                    let seller_boundary = format_seller_boundary(locale.as_deref(), product.seller_id.as_deref());
                    view! {
                        <article class="rounded-2xl border border-border bg-background p-5">
                            <div class="space-y-2">
                                <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{product.product_type.unwrap_or_else(|| t(locale.as_deref(), "pricing.selected.catalog", "catalog"))}</div>
                                <h4 class="text-base font-semibold text-card-foreground">{product.title}</h4>
                                <p class="text-sm text-muted-foreground">{product.vendor.unwrap_or_else(|| t(locale.as_deref(), "pricing.list.vendorFallback", "Independent label"))}</p>
                                <p class="text-xs text-muted-foreground">{seller_boundary}</p>
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

fn pricing_translation_for_locale<'a>(
    translations: &'a [crate::model::PricingProductTranslation],
    requested_locale: Option<&str>,
) -> Option<&'a crate::model::PricingProductTranslation> {
    requested_locale
        .and_then(|locale| {
            translations
                .iter()
                .find(|translation| locale_tags_match(&translation.locale, locale))
        })
        .or_else(|| translations.first())
}

fn format_seller_boundary(locale: Option<&str>, seller_id: Option<&str>) -> String {
    match seller_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(seller_id) => format!(
            "{}: {seller_id}",
            t(locale, "pricing.common.sellerId", "seller id")
        ),
        None => t(
            locale,
            "pricing.common.sellerUnassigned",
            "seller id: unassigned",
        ),
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
                    "{} {} ({}){}",
                    price.currency_code,
                    price.amount,
                    t(locale, "pricing.variant.compareAt", "compare-at {value}")
                        .replace("{value}", compare),
                    format_discount_suffix(price.discount_percent.as_deref()),
                )
            } else {
                format!(
                    "{} {}{}",
                    price.currency_code,
                    price.amount,
                    format_discount_suffix(price.discount_percent.as_deref())
                )
            }
        })
        .collect::<Vec<_>>()
        .join(" • ")
}

fn pricing_health_label(locale: Option<&str>, variant: &PricingVariant) -> String {
    if variant.effective_price.is_some() {
        return t(locale, "pricing.health.effective", "effective");
    }
    if variant.prices.is_empty() {
        return t(locale, "pricing.health.missing", "missing");
    }
    if variant.prices.iter().any(|price| price.on_sale) {
        return t(locale, "pricing.health.sale", "sale");
    }
    t(locale, "pricing.health.covered", "covered")
}

fn pricing_health_badge(variant: &PricingVariant) -> &'static str {
    if variant.effective_price.is_some() {
        "border-primary/30 text-primary"
    } else if variant.prices.is_empty() {
        "border-destructive/30 text-destructive"
    } else if variant.prices.iter().any(|price| price.on_sale) {
        "border-emerald-500/30 text-emerald-700"
    } else {
        "border-border text-muted-foreground"
    }
}

fn format_price_list_option_label(locale: Option<&str>, option: &PricingPriceListOption) -> String {
    let mut label = format!(
        "{} ({} {})",
        option.name,
        t(locale, "pricing.selected.priceListTypeLabel", "type"),
        option.list_type
    );
    if option.rule_kind.as_deref() == Some("percentage_discount") {
        if let Some(adjustment_percent) = option.adjustment_percent.as_deref() {
            label.push_str(format!(" | -{adjustment_percent}%").as_str());
        }
    }
    label
}

fn resolve_price_list_label(
    locale: Option<&str>,
    price_list_id: Option<&str>,
    options: &[PricingPriceListOption],
    base_fallback_key: &str,
    base_fallback: &str,
) -> String {
    let Some(price_list_id) = price_list_id.filter(|value| !value.trim().is_empty()) else {
        return t(locale, base_fallback_key, base_fallback);
    };

    options
        .iter()
        .find(|option| option.id == price_list_id)
        .map(|option| format_price_list_option_label(locale, option))
        .unwrap_or_else(|| price_list_id.to_string())
}

fn format_effective_context(
    locale: Option<&str>,
    context: &PricingResolutionContext,
    price_list_options: &[PricingPriceListOption],
) -> String {
    let region = context.region_id.clone().unwrap_or_else(|| {
        t(
            locale,
            "pricing.selected.globalRegionFallback",
            "global region",
        )
    });
    let price_list = resolve_price_list_label(
        locale,
        context.price_list_id.as_deref(),
        price_list_options,
        "pricing.selected.basePriceListFallback",
        "base prices",
    );
    let mut parts = vec![
        format!(
            "{} {}",
            t(locale, "pricing.selected.currencyLabel", "currency"),
            context.currency_code
        ),
        format!(
            "{} {}",
            t(locale, "pricing.selected.regionLabel", "region"),
            region
        ),
        format!(
            "{} {}",
            t(locale, "pricing.selected.priceListLabel", "price list"),
            price_list
        ),
    ];
    if let Some(channel_scope) = format_channel_scope_text(
        locale,
        context.channel_id.as_deref(),
        context.channel_slug.as_deref(),
    ) {
        parts.push(channel_scope);
    }
    parts.push(format!(
        "{} {}",
        t(locale, "pricing.selected.quantityLabel", "qty"),
        context.quantity
    ));
    parts.join(" | ")
}

fn format_channel_scope_text(
    locale: Option<&str>,
    channel_id: Option<&str>,
    channel_slug: Option<&str>,
) -> Option<String> {
    let channel_slug = channel_slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let channel_id = channel_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    if channel_slug.is_none() && channel_id.is_none() {
        return None;
    }

    let channel_label = t(locale, "pricing.selected.channelLabel", "channel");
    match (channel_slug, channel_id) {
        (Some(channel_slug), Some(channel_id)) => {
            Some(format!("{channel_label} {channel_slug} ({channel_id})"))
        }
        (Some(channel_slug), None) => Some(format!("{channel_label} {channel_slug}")),
        (None, Some(channel_id)) => Some(format!("{channel_label} {channel_id}")),
        (None, None) => None,
    }
}

fn format_channel_option_label(locale: Option<&str>, option: &PricingChannelOption) -> String {
    let mut label = format!("{} ({})", option.name, option.slug);
    if option.is_default {
        label.push_str(format!(" | {}", t(locale, "pricing.channel.default", "default")).as_str());
    }
    if !option.is_active {
        label
            .push_str(format!(" | {}", t(locale, "pricing.channel.inactive", "inactive")).as_str());
    }
    label
}

fn format_effective_price(
    locale: Option<&str>,
    price: &crate::model::PricingEffectivePrice,
) -> String {
    let base = if let Some(compare_at_amount) = price.compare_at_amount.as_deref() {
        format!(
            "{} {} ({}){}",
            price.currency_code,
            price.amount,
            t(locale, "pricing.variant.compareAt", "compare-at {value}")
                .replace("{value}", compare_at_amount),
            format_discount_suffix(price.discount_percent.as_deref()),
        )
    } else {
        format!(
            "{} {}{}",
            price.currency_code,
            price.amount,
            format_discount_suffix(price.discount_percent.as_deref())
        )
    };

    let scope = match (price.min_quantity, price.max_quantity) {
        (Some(min_quantity), Some(max_quantity)) => format!(
            "{} {}-{}",
            t(locale, "pricing.selected.quantityRange", "tier"),
            min_quantity,
            max_quantity
        ),
        (Some(min_quantity), None) => format!(
            "{} {}+",
            t(locale, "pricing.selected.quantityRange", "tier"),
            min_quantity
        ),
        _ => t(
            locale,
            "pricing.selected.quantityDefault",
            "default quantity",
        )
        .to_string(),
    };

    format!(
        "{} | {} {}",
        base,
        t(locale, "pricing.selected.effectiveLabel", "effective"),
        scope
    )
}

fn format_discount_suffix(discount_percent: Option<&str>) -> String {
    discount_percent
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" (-{value}%)"))
        .unwrap_or_default()
}

fn selector_badge_class(active: bool) -> &'static str {
    if active {
        "inline-flex items-center rounded-full border border-primary/30 bg-primary/5 px-3 py-1 text-xs font-medium text-primary"
    } else {
        "inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground transition hover:border-primary/30 hover:text-primary"
    }
}

#[derive(Clone, Copy, Default)]
struct PricingRouteParams<'a> {
    selected_handle: Option<&'a str>,
    currency_code: Option<&'a str>,
    region_id: Option<&'a str>,
    price_list_id: Option<&'a str>,
    channel_id: Option<&'a str>,
    channel_slug: Option<&'a str>,
    quantity: Option<i32>,
}

fn build_pricing_route_href(module_route_base: &str, params: PricingRouteParams<'_>) -> String {
    let mut query_params = Vec::new();

    if let Some(handle) = params
        .selected_handle
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(format!("handle={handle}"));
    }
    if let Some(currency_code) = params
        .currency_code
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(format!("currency={currency_code}"));
    }
    if let Some(region_id) = params
        .region_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(format!("region_id={region_id}"));
    }
    if let Some(price_list_id) = params
        .price_list_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(format!("price_list_id={price_list_id}"));
    }
    if let Some(channel_id) = params
        .channel_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(format!("channel_id={channel_id}"));
    }
    if let Some(channel_slug) = params
        .channel_slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(format!("channel_slug={channel_slug}"));
    }
    if let Some(quantity) = params.quantity.filter(|value| *value > 0) {
        query_params.push(format!("quantity={quantity}"));
    }

    if query_params.is_empty() {
        module_route_base.to_string()
    } else {
        format!("{module_route_base}?{}", query_params.join("&"))
    }
}

fn build_product_storefront_href(module_route_base: &str, handle: Option<&str>) -> String {
    match handle.map(str::trim).filter(|value| !value.is_empty()) {
        Some(handle) => format!("{module_route_base}?handle={handle}"),
        None => module_route_base.to_string(),
    }
}
