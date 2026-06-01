use leptos::prelude::*;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;

use crate::core::{
    build_pricing_route_href, build_product_storefront_href, format_channel_option_label,
    format_effective_context, format_effective_price, format_price_list_option_label,
    format_seller_boundary, format_variant_identity, format_variant_prices, pricing_health_badge,
    pricing_health_label, pricing_translation_for_locale, selector_badge_class, summarize_pricing,
    PricingRouteParams, StorefrontPricingQuery,
};
use crate::i18n::t;
use crate::model::{
    PricingChannelOption, PricingPriceListOption, PricingProductDetail, PricingProductListItem,
    PricingResolutionContext, StorefrontPricingData,
};
use crate::transport;

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
            transport::fetch_storefront_pricing(StorefrontPricingQuery {
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
