mod api;
mod i18n;
mod model;

use leptos::prelude::*;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;
use rustok_core::locale_tags_match;

use crate::i18n::t;
use crate::model::{
    ProductDetail, ProductListItem, ProductPricingContext, ProductPricingDetail,
    StorefrontProductsData,
};

#[component]
pub fn ProductView() -> impl IntoView {
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
    let badge = t(selected_locale.as_deref(), "product.badge", "product");
    let title = t(
        selected_locale.as_deref(),
        "product.title",
        "Published catalog from the product module",
    );
    let subtitle = t(
        selected_locale.as_deref(),
        "product.subtitle",
        "This storefront route reads published catalog data through the product-owned package, with GraphQL kept as a fallback path.",
    );
    let load_error = t(
        selected_locale.as_deref(),
        "product.error.load",
        "Failed to load storefront product data",
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
            api::fetch_storefront_products(
                handle,
                locale,
                currency_code,
                region_id,
                price_list_id,
                channel_id,
                channel_slug,
                quantity,
            )
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
                        let resource = resource.clone();
                        let load_error = load_error.clone();
                        Suspend::new(async move {
                            match resource.await {
                                Ok(data) => view! { <ProductShowcase data /> }.into_any(),
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
fn ProductShowcase(data: StorefrontProductsData) -> impl IntoView {
    view! {
        <div class="grid gap-6 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
            <SelectedProductCard
                product=data.selected_product
                pricing=data.selected_pricing
                resolution_context=data.resolution_context
                selected_handle=data.selected_handle
            />
            <CatalogRail items=data.products.items total=data.products.total />
        </div>
    }
}

#[component]
fn SelectedProductCard(
    product: Option<ProductDetail>,
    pricing: Option<ProductPricingDetail>,
    resolution_context: Option<ProductPricingContext>,
    selected_handle: Option<String>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let Some(product) = product else {
        return view! {
            <article class="rounded-3xl border border-dashed border-border p-8">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {t(locale.as_deref(), "product.selected.emptyTitle", "No published product selected")}
                </h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {t(locale.as_deref(), "product.selected.emptyBody", "Publish a product from the product admin package or open one with `?handle=`.")}
                </p>
            </article>
        }.into_any();
    };

    let translation = product_translation_for_locale(
        product.translations.as_slice(),
        locale.as_deref(),
    )
    .cloned();
    let variant = product.variants.first().cloned();
    let title = translation
        .as_ref()
        .map(|item| item.title.clone())
        .unwrap_or_else(|| {
            t(
                locale.as_deref(),
                "product.selected.untitled",
                "Untitled product",
            )
        });
    let description = translation
        .as_ref()
        .and_then(|item| item.description.clone())
        .unwrap_or_else(|| {
            t(
                locale.as_deref(),
                "product.selected.noDescription",
                "No localized merchandising copy yet.",
            )
        });
    let catalog_snapshot = variant
        .as_ref()
        .and_then(|item| item.prices.first())
        .map(|item| {
            format_product_price(
                locale.as_deref(),
                item.currency_code.as_str(),
                item.amount.as_str(),
                item.compare_at_amount.as_deref(),
                None,
            )
        })
        .unwrap_or_else(|| {
            t(
                locale.as_deref(),
                "product.selected.noPrice",
                "No pricing yet",
            )
        });
    let pricing_preview = format_pricing_preview(locale.as_deref(), pricing.as_ref());
    let inventory = variant
        .as_ref()
        .map(|item| item.inventory_quantity)
        .unwrap_or(0);
    let seller_boundary = format_seller_boundary(locale.as_deref(), product.seller_id.as_deref());
    let pricing_href = build_storefront_pricing_href(
        route_context.module_route_base("pricing").as_str(),
        selected_handle
            .as_deref()
            .or_else(|| translation.as_ref().map(|item| item.handle.as_str())),
        resolution_context.as_ref(),
        variant.as_ref(),
    );

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                <span>{product.product_type.unwrap_or_else(|| t(locale.as_deref(), "product.selected.catalog", "catalog"))}</span>
                <span>"|"</span>
                <span>{product.vendor.unwrap_or_else(|| t(locale.as_deref(), "product.selected.vendorFallback", "independent label"))}</span>
                <span>"|"</span>
                <span>{product.published_at.unwrap_or_else(|| t(locale.as_deref(), "product.selected.unscheduled", "scheduled later"))}</span>
            </div>
            <p class="mt-3 text-xs font-medium text-muted-foreground">{seller_boundary}</p>
            <h3 class="mt-4 text-3xl font-semibold text-foreground">{title}</h3>
            <p class="mt-4 text-sm leading-7 text-muted-foreground">{description}</p>
            {resolution_context.as_ref().map(|context| view! {
                <div class="mt-4 inline-flex flex-wrap items-center gap-2 rounded-2xl border border-primary/20 bg-primary/5 px-4 py-2 text-xs text-primary">
                    <span class="font-semibold uppercase tracking-[0.16em]">
                        {t(locale.as_deref(), "product.selected.previewContext", "pricing preview")}
                    </span>
                    <span>{format_pricing_context(locale.as_deref(), context)}</span>
                </div>
            })}
            <p class="mt-4 text-xs text-muted-foreground">
                {t(
                    locale.as_deref(),
                    "product.selected.pricingOwnershipNote",
                    "Catalog snapshot stays product-owned; resolved pricing comes from the pricing module preview.",
                )}
            </p>
            <div class="mt-6 grid gap-3 md:grid-cols-3">
                <MetricCard title=t(locale.as_deref(), "product.selected.catalogSnapshot", "Catalog snapshot") value=catalog_snapshot />
                <MetricCard title=t(locale.as_deref(), "product.selected.pricingPreview", "Pricing module preview") value=pricing_preview />
                <MetricCard title=t(locale.as_deref(), "product.selected.inventory", "Inventory") value=inventory.to_string() />
            </div>
            <div class="mt-4">
                <a
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent"
                    href=pricing_href
                >
                    {t(
                        locale.as_deref(),
                        "product.selected.openPricing",
                        "Open pricing module",
                    )}
                </a>
            </div>
        </article>
    }.into_any()
}

#[component]
fn CatalogRail(items: Vec<ProductListItem>, total: u64) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let route_segment = route_context
        .route_segment
        .as_ref()
        .cloned()
        .unwrap_or_else(|| "products".to_string());
    let module_route_base = route_context.module_route_base(route_segment.as_str());

    if items.is_empty() {
        return view! { <article class="rounded-3xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{t(locale.as_deref(), "product.list.empty", "No published products are available yet.")}</article> }.into_any();
    }

    view! {
        <div class="space-y-4">
            <div class="flex items-center justify-between gap-3">
                <h3 class="text-lg font-semibold text-card-foreground">{t(locale.as_deref(), "product.list.title", "Published products")}</h3>
                <span class="text-sm text-muted-foreground">
                    {t(locale.as_deref(), "product.list.total", "{count} total").replace("{count}", &total.to_string())}
                </span>
            </div>
            <div class="space-y-3">
                {items.into_iter().map(|product| {
                    let locale = locale.clone();
                    let href = format!("{module_route_base}?handle={}", product.handle);
                    let seller_boundary = format_seller_boundary(locale.as_deref(), product.seller_id.as_deref());
                    view! {
                        <article class="rounded-2xl border border-border bg-background p-5">
                            <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{product.product_type.unwrap_or_else(|| t(locale.as_deref(), "product.selected.catalog", "catalog"))}</div>
                            <h4 class="mt-2 text-base font-semibold text-card-foreground">{product.title}</h4>
                            <p class="mt-2 text-sm text-muted-foreground">{product.vendor.unwrap_or_else(|| t(locale.as_deref(), "product.list.vendorFallback", "Independent label"))}</p>
                            <p class="mt-1 text-xs text-muted-foreground">{seller_boundary}</p>
                            <div class="mt-4 flex items-center justify-between gap-3">
                                <span class="text-xs text-muted-foreground">{product.published_at.unwrap_or(product.created_at)}</span>
                                <a class="inline-flex text-sm font-medium text-primary hover:underline" href=href>{t(locale.as_deref(), "product.list.open", "Open")}</a>
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

fn format_pricing_preview(locale: Option<&str>, pricing: Option<&ProductPricingDetail>) -> String {
    let Some(pricing) = pricing else {
        return t(
            locale,
            "product.selected.noPricingPreview",
            "Pricing module preview is unavailable.",
        );
    };

    let Some(variant) = pricing.variants.first() else {
        return t(locale, "product.selected.noPrice", "No pricing yet");
    };

    if let Some(price) = variant.effective_price.as_ref() {
        let mut label = format_product_price(
            locale,
            price.currency_code.as_str(),
            price.amount.as_str(),
            price.compare_at_amount.as_deref(),
            price.discount_percent.as_deref(),
        );
        if let Some(scope) = format_pricing_scope(
            locale,
            price.price_list_id.as_deref(),
            price.channel_slug.as_deref(),
            price.channel_id.as_deref(),
        ) {
            label.push_str(format!(" | {scope}").as_str());
        }
        return label;
    }

    variant
        .prices
        .first()
        .map(|price| {
            format_product_price(
                locale,
                price.currency_code.as_str(),
                price.amount.as_str(),
                price.compare_at_amount.as_deref(),
                price.discount_percent.as_deref(),
            )
        })
        .unwrap_or_else(|| t(locale, "product.selected.noPrice", "No pricing yet"))
}

fn product_translation_for_locale<'a>(
    translations: &'a [crate::model::ProductTranslation],
    requested_locale: Option<&str>,
) -> Option<&'a crate::model::ProductTranslation> {
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
            t(locale, "product.common.sellerId", "seller id")
        ),
        None => t(
            locale,
            "product.common.sellerUnassigned",
            "seller id: unassigned",
        ),
    }
}

fn format_product_price(
    locale: Option<&str>,
    currency_code: &str,
    amount: &str,
    compare_at_amount: Option<&str>,
    discount_percent: Option<&str>,
) -> String {
    let mut label = if let Some(compare_at_amount) = compare_at_amount {
        format!(
            "{} {} ({})",
            currency_code,
            amount,
            t(locale, "product.selected.compareAt", "compare-at {value}")
                .replace("{value}", compare_at_amount),
        )
    } else {
        format!("{currency_code} {amount}")
    };

    if let Some(discount_percent) = discount_percent.filter(|value| !value.trim().is_empty()) {
        label.push_str(format!(" (-{discount_percent}%)").as_str());
    }

    label
}

fn format_pricing_scope(
    locale: Option<&str>,
    price_list_id: Option<&str>,
    channel_slug: Option<&str>,
    channel_id: Option<&str>,
) -> Option<String> {
    let price_list_id = price_list_id.filter(|value| !value.trim().is_empty());
    let channel_slug = channel_slug.filter(|value| !value.trim().is_empty());
    let channel_id = channel_id.filter(|value| !value.trim().is_empty());

    if price_list_id.is_none() && channel_slug.is_none() && channel_id.is_none() {
        return None;
    }

    let mut parts = Vec::new();
    if let Some(price_list_id) = price_list_id {
        parts.push(t(locale, "product.selected.priceList", "price list") + " " + price_list_id);
    }
    match (channel_slug, channel_id) {
        (Some(channel_slug), Some(channel_id)) => parts.push(
            t(locale, "product.selected.channel", "channel")
                + " "
                + channel_slug
                + " ("
                + channel_id
                + ")",
        ),
        (Some(channel_slug), None) => {
            parts.push(t(locale, "product.selected.channel", "channel") + " " + channel_slug)
        }
        (None, Some(channel_id)) => {
            parts.push(t(locale, "product.selected.channel", "channel") + " " + channel_id)
        }
        (None, None) => {}
    }

    Some(parts.join(" | "))
}

fn format_pricing_context(locale: Option<&str>, context: &ProductPricingContext) -> String {
    let mut parts = vec![
        format!(
            "{} {}",
            t(locale, "product.selected.currency", "currency"),
            context.currency_code
        ),
        format!(
            "{} {}",
            t(locale, "product.selected.quantity", "qty"),
            context.quantity
        ),
    ];

    if let Some(region_id) = context.region_id.as_deref() {
        parts.push(format!(
            "{} {}",
            t(locale, "product.selected.region", "region"),
            region_id
        ));
    }
    if let Some(scope) = format_pricing_scope(
        locale,
        context.price_list_id.as_deref(),
        context.channel_slug.as_deref(),
        context.channel_id.as_deref(),
    ) {
        parts.push(scope);
    }

    parts.join(" | ")
}

fn build_storefront_pricing_href(
    module_route_base: &str,
    handle: Option<&str>,
    resolution_context: Option<&ProductPricingContext>,
    variant: Option<&crate::model::ProductVariant>,
) -> String {
    let mut params = Vec::new();
    if let Some(handle) = handle.map(str::trim).filter(|value| !value.is_empty()) {
        params.push(format!("handle={handle}"));
    }

    let fallback_currency = variant
        .and_then(|item| item.prices.first())
        .map(|price| price.currency_code.as_str());
    let currency_code = resolution_context
        .map(|context| context.currency_code.as_str())
        .or(fallback_currency);
    if let Some(currency_code) = currency_code
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        params.push(format!("currency={currency_code}"));
    }
    if let Some(region_id) = resolution_context
        .and_then(|context| context.region_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        params.push(format!("region_id={region_id}"));
    }
    if let Some(price_list_id) = resolution_context
        .and_then(|context| context.price_list_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        params.push(format!("price_list_id={price_list_id}"));
    }
    if let Some(channel_id) = resolution_context
        .and_then(|context| context.channel_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        params.push(format!("channel_id={channel_id}"));
    }
    if let Some(channel_slug) = resolution_context
        .and_then(|context| context.channel_slug.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        params.push(format!("channel_slug={channel_slug}"));
    }
    if let Some(quantity) = resolution_context
        .map(|context| context.quantity)
        .filter(|value| *value > 0)
    {
        params.push(format!("quantity={quantity}"));
    }

    if params.is_empty() {
        module_route_base.to_string()
    } else {
        format!("{module_route_base}?{}", params.join("&"))
    }
}
