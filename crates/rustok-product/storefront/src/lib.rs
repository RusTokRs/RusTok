mod api;
mod core;
mod i18n;
mod model;
mod transport;

use crate::core::{
    build_storefront_pricing_href, build_storefront_route_input, format_pricing_context,
    format_pricing_preview, format_product_price, format_seller_boundary,
    product_translation_for_locale,
};
use crate::i18n::t;
use crate::model::{
    ProductDetail, ProductListItem, ProductPricingContext, ProductPricingDetail,
    StorefrontProductsData,
};
use leptos::prelude::*;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;

#[component]
pub fn ProductView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let route_input = build_storefront_route_input(
        read_route_query_value(&route_context, "handle"),
        route_context.locale.clone(),
        read_route_query_value(&route_context, "currency"),
        read_route_query_value(&route_context, "region_id"),
        read_route_query_value(&route_context, "price_list_id"),
        read_route_query_value(&route_context, "channel_id"),
        read_route_query_value(&route_context, "channel_slug"),
        read_route_query_value(&route_context, "quantity"),
    );
    let selected_handle = route_input.handle.clone();
    let selected_locale = route_input.locale.clone();
    let selected_currency_code = route_input.currency_code.clone();
    let selected_region_id = route_input.region_id.clone();
    let selected_price_list_id = route_input.price_list_id.clone();
    let selected_channel_id = route_input.channel_id.clone();
    let selected_channel_slug = route_input.channel_slug.clone();
    let selected_quantity = route_input.quantity;
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
            transport::fetch_products(
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
                        let resource = resource;
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

    let translation =
        product_translation_for_locale(product.translations.as_slice(), locale.as_deref()).cloned();
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
