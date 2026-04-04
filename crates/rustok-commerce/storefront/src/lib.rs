mod api;
mod i18n;
mod model;

use leptos::prelude::*;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{ProductDetail, ProductListItem, StorefrontCommerceData};

#[component]
pub fn CommerceView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_handle = route_context.query_value("handle").map(ToOwned::to_owned);
    let selected_locale = route_context.locale.clone();
    let badge = t(selected_locale.as_deref(), "commerce.badge", "commerce");
    let title = t(
        selected_locale.as_deref(),
        "commerce.title",
        "Published catalog from the module package",
    );
    let subtitle = t(
        selected_locale.as_deref(),
        "commerce.subtitle",
        "The storefront now reads published products through the module-owned GraphQL contract with no commerce-specific host wiring.",
    );
    let load_error = t(
        selected_locale.as_deref(),
        "commerce.error.load",
        "Failed to load commerce storefront data",
    );

    let resource = Resource::new_blocking(
        move || (selected_handle.clone(), selected_locale.clone()),
        move |(handle, locale)| async move { api::fetch_storefront_commerce(handle, locale).await },
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
        <div class="grid gap-6 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
            <SelectedProductCard product=data.selected_product />
            <CatalogRail items=data.products.items total=data.products.total />
        </div>
    }
}

#[component]
fn SelectedProductCard(product: Option<ProductDetail>) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let Some(product) = product else {
        return view! {
            <article class="rounded-3xl border border-dashed border-border p-8">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {t(
                        locale.as_deref(),
                        "commerce.selected.emptyTitle",
                        "No published product selected",
                    )}
                </h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {t(
                        locale.as_deref(),
                        "commerce.selected.emptyBody",
                        "Publish a product from the commerce admin package or open one with `?handle=`.",
                    )}
                </p>
            </article>
        }
        .into_any();
    };

    let translation = product.translations.first().cloned();
    let variant = product.variants.first().cloned();
    let title = translation
        .as_ref()
        .map(|item| item.title.clone())
        .unwrap_or_else(|| t(locale.as_deref(), "commerce.selected.untitled", "Untitled product"));
    let description = translation.and_then(|item| item.description).unwrap_or_else(|| {
        t(
            locale.as_deref(),
            "commerce.selected.noDescription",
            "No localized merchandising copy yet.",
        )
    });
    let price = variant
        .as_ref()
        .and_then(|item| item.prices.first())
        .map(|item| {
            if let Some(compare) = &item.compare_at_amount {
                format!(
                    "{} {} ({})",
                    item.currency_code,
                    item.amount,
                    t(
                        locale.as_deref(),
                        "commerce.selected.compareAt",
                        "compare-at {value}",
                    )
                    .replace("{value}", compare),
                )
            } else {
                format!("{} {}", item.currency_code, item.amount)
            }
        })
        .unwrap_or_else(|| t(locale.as_deref(), "commerce.selected.noPrice", "No pricing yet"));
    let inventory = variant.as_ref().map(|item| item.inventory_quantity).unwrap_or(0);

    view! {
        <article class="rounded-3xl border border-border bg-background p-8">
            <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                <span>{product.product_type.unwrap_or_else(|| t(locale.as_deref(), "commerce.selected.catalog", "catalog"))}</span>
                <span>"·"</span>
                <span>{product.vendor.unwrap_or_else(|| t(locale.as_deref(), "commerce.selected.vendorFallback", "independent label"))}</span>
                <span>"·"</span>
                <span>{product.published_at.unwrap_or_else(|| t(locale.as_deref(), "commerce.selected.unscheduled", "scheduled later"))}</span>
            </div>
            <h3 class="mt-4 text-3xl font-semibold text-foreground">{title}</h3>
            <p class="mt-4 text-sm leading-7 text-muted-foreground">{description}</p>
            <div class="mt-6 grid gap-3 md:grid-cols-2">
                <MetricCard title=t(locale.as_deref(), "commerce.selected.price", "Price") value=price />
                <MetricCard title=t(locale.as_deref(), "commerce.selected.inventory", "Inventory") value=inventory.to_string() />
            </div>
        </article>
    }
    .into_any()
}

#[component]
fn CatalogRail(items: Vec<ProductListItem>, total: u64) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let route_segment = route_context
        .route_segment
        .as_ref()
        .cloned()
        .unwrap_or_else(|| "commerce".to_string());
    let module_route_base = route_context.module_route_base(route_segment.as_str());

    if items.is_empty() {
        return view! { <article class="rounded-3xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{t(locale.as_deref(), "commerce.list.empty", "No published products are available yet.")}</article> }.into_any();
    }

    view! {
        <div class="space-y-4">
            <div class="flex items-center justify-between gap-3">
                <h3 class="text-lg font-semibold text-card-foreground">{t(locale.as_deref(), "commerce.list.title", "Published products")}</h3>
                <span class="text-sm text-muted-foreground">
                    {t(locale.as_deref(), "commerce.list.total", "{count} total").replace("{count}", &total.to_string())}
                </span>
            </div>
            <div class="space-y-3">
                {items.into_iter().map(|product| {
                    let locale = locale.clone();
                    let href = format!("{module_route_base}?handle={}", product.handle);
                    view! {
                        <article class="rounded-2xl border border-border bg-background p-5">
                            <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{product.product_type.unwrap_or_else(|| t(locale.as_deref(), "commerce.selected.catalog", "catalog"))}</div>
                            <h4 class="mt-2 text-base font-semibold text-card-foreground">{product.title}</h4>
                            <p class="mt-2 text-sm text-muted-foreground">{product.vendor.unwrap_or_else(|| t(locale.as_deref(), "commerce.list.vendorFallback", "Independent label"))}</p>
                            <div class="mt-4 flex items-center justify-between gap-3">
                                <span class="text-xs text-muted-foreground">{product.published_at.unwrap_or(product.created_at)}</span>
                                <a class="inline-flex text-sm font-medium text-primary hover:underline" href=href>{t(locale.as_deref(), "commerce.list.open", "Open")}</a>
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
