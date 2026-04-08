use std::collections::BTreeSet;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{
    PricingAdminBootstrap, PricingPrice, PricingProductDetail, PricingProductListItem,
    PricingVariant,
};

#[component]
pub fn PricingAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let initial_locale = ui_locale.clone().unwrap_or_else(|| "en".to_string());
    let token = use_token();
    let tenant = use_tenant();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (selected_id, set_selected_id) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<PricingProductDetail>::None);
    let (search, set_search) = signal(String::new());
    let (status_filter, set_status_filter) = signal(String::new());
    let (locale, _set_locale) = signal(initial_locale.clone());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    let bootstrap = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            crate::api::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let products = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                locale.get(),
                search.get(),
                status_filter.get(),
            )
        },
        move |(token_value, tenant_value, _, locale_value, search_value, status_value)| async move {
            let bootstrap =
                crate::api::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            crate::api::fetch_products(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                locale_value,
                text_or_none(search_value),
                text_or_none(status_value),
            )
            .await
        },
    );

    let bootstrap_loading_label = t(
        ui_locale.as_deref(),
        "pricing.error.bootstrapLoading",
        "Bootstrap is still loading.",
    );
    let load_product_error_label = t(
        ui_locale.as_deref(),
        "pricing.error.loadProduct",
        "Failed to load pricing detail",
    );
    let product_not_found_label = t(
        ui_locale.as_deref(),
        "pricing.error.productNotFound",
        "Product not found.",
    );
    let load_products_error_label = t(
        ui_locale.as_deref(),
        "pricing.error.loadProducts",
        "Failed to load pricing feed",
    );

    let open_bootstrap_loading_label = bootstrap_loading_label.clone();
    let open_load_product_error_label = load_product_error_label.clone();
    let open_product_not_found_label = product_not_found_label.clone();
    let open_product = Callback::new(move |product_id: String| {
        let Some(PricingAdminBootstrap { current_tenant }) =
            bootstrap.get_untracked().and_then(Result::ok)
        else {
            set_error.set(Some(open_bootstrap_loading_label.clone()));
            return;
        };

        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let locale_value = locale.get_untracked();
        let not_found_label = open_product_not_found_label.clone();
        let load_error_label = open_load_product_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match crate::api::fetch_product(
                token_value,
                tenant_value,
                current_tenant.id,
                product_id,
                locale_value,
            )
            .await
            {
                Ok(Some(product)) => {
                    set_selected_id.set(Some(product.id.clone()));
                    set_selected.set(Some(product));
                }
                Ok(None) => set_error.set(Some(not_found_label)),
                Err(err) => set_error.set(Some(format!("{load_error_label}: {err}"))),
            }
            set_busy.set(false);
        });
    });

    let ui_locale_for_list = ui_locale.clone();
    let ui_locale_for_list_status = ui_locale.clone();
    let ui_locale_for_detail = ui_locale.clone();
    let ui_locale_for_variants = ui_locale.clone();
    let ui_locale_for_empty = ui_locale.clone();

    view! {
        <section class="space-y-6">
            <header class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-3">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                        {t(ui_locale.as_deref(), "pricing.badge", "pricing")}
                    </span>
                    <h2 class="text-2xl font-semibold text-card-foreground">
                        {t(ui_locale.as_deref(), "pricing.title", "Pricing Control")}
                    </h2>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        {t(ui_locale.as_deref(), "pricing.subtitle", "Module-owned pricing read-side surface for price visibility, sale markers and currency coverage while dedicated pricing mutations are still being split from the umbrella transport.")}
                    </p>
                </div>
            </header>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,0.95fr)_minmax(0,1.15fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">
                                {t(ui_locale.as_deref(), "pricing.list.title", "Pricing Feed")}
                            </h3>
                            <p class="text-sm text-muted-foreground">
                                {t(ui_locale.as_deref(), "pricing.list.subtitle", "Search the catalog and open a product to inspect variant-level price coverage owned by the pricing boundary.")}
                            </p>
                        </div>
                        <div class="grid gap-3 md:grid-cols-[minmax(0,1fr)_180px_auto]">
                            <input
                                class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                placeholder=t(ui_locale.as_deref(), "pricing.list.search", "Search title")
                                prop:value=move || search.get()
                                on:input=move |ev| set_search.set(event_target_value(&ev))
                            />
                            <select
                                class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                prop:value=move || status_filter.get()
                                on:change=move |ev| set_status_filter.set(event_target_value(&ev))
                            >
                                <option value="">{t(ui_locale.as_deref(), "pricing.filter.allStatuses", "All statuses")}</option>
                                <option value="DRAFT">{t(ui_locale.as_deref(), "pricing.status.draft", "Draft")}</option>
                                <option value="ACTIVE">{t(ui_locale.as_deref(), "pricing.status.active", "Active")}</option>
                                <option value="ARCHIVED">{t(ui_locale.as_deref(), "pricing.status.archived", "Archived")}</option>
                            </select>
                            <button
                                type="button"
                                class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                disabled=move || busy.get()
                                on:click=move |_| set_refresh_nonce.update(|value| *value += 1)
                            >
                                {t(ui_locale.as_deref(), "pricing.action.refresh", "Refresh")}
                            </button>
                        </div>
                    </div>

                    <div class="mt-5 space-y-3">
                        {move || match products.get() {
                            None => view! {
                                <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                                    {t(ui_locale_for_list.as_deref(), "pricing.loading", "Loading pricing feed...")}
                                </div>
                            }.into_any(),
                            Some(Err(err)) => view! {
                                <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                    {format!("{load_products_error_label}: {err}")}
                                </div>
                            }.into_any(),
                            Some(Ok(list)) if list.items.is_empty() => view! {
                                <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                                    {t(ui_locale_for_list.as_deref(), "pricing.list.empty", "No products match the current filters.")}
                                </div>
                            }.into_any(),
                            Some(Ok(list)) => view! {
                                <>
                                    {list.items.into_iter().map(|product| {
                                        let open_id = product.id.clone();
                                        let selected_marker = product.id.clone();
                                        let item_locale = ui_locale_for_list_status.clone();
                                        let item_locale_for_meta = item_locale.clone();
                                        let item_locale_for_profile = item_locale.clone();
                                        let shipping_profile = product.shipping_profile_slug.clone();
                                        let profile_label = shipping_profile
                                            .unwrap_or_else(|| t(item_locale_for_profile.as_deref(), "pricing.common.unassigned", "unassigned"));
                                        view! {
                                            <article class=move || {
                                                if selected_id.get() == Some(selected_marker.clone()) {
                                                    "rounded-2xl border border-primary/40 bg-background p-5 shadow-sm"
                                                } else {
                                                    "rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40"
                                                }
                                            }>
                                                <div class="flex items-start justify-between gap-3">
                                                    <div class="space-y-2">
                                                        <div class="flex flex-wrap items-center gap-2">
                                                            <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", status_badge(product.status.as_str()))>
                                                                {localized_product_status(item_locale.as_deref(), product.status.as_str())}
                                                            </span>
                                                            <span class="inline-flex rounded-full border border-border px-3 py-1 text-xs text-muted-foreground">
                                                                {profile_label.clone()}
                                                            </span>
                                                        </div>
                                                        <h4 class="text-base font-semibold text-card-foreground">{product.title.clone()}</h4>
                                                        <p class="text-sm text-muted-foreground">{format_product_meta(item_locale_for_meta.as_deref(), &product)}</p>
                                                    </div>
                                                    <button
                                                        type="button"
                                                        class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                                        disabled=move || busy.get()
                                                        on:click=move |_| open_product.run(open_id.clone())
                                                    >
                                                        {t(item_locale.as_deref(), "pricing.action.open", "Open")}
                                                    </button>
                                                </div>
                                            </article>
                                        }
                                    }).collect_view()}
                                </>
                            }.into_any(),
                        }}
                    </div>
                </section>

                <section class="space-y-6 rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="space-y-2">
                        <h3 class="text-lg font-semibold text-card-foreground">
                            {t(ui_locale.as_deref(), "pricing.detail.title", "Pricing Detail")}
                        </h3>
                        <p class="text-sm text-muted-foreground">
                            {t(ui_locale.as_deref(), "pricing.detail.subtitle", "Inspect currency coverage, compare-at pricing and sale visibility from the pricing-owned route.")}
                        </p>
                    </div>

                    <Show when=move || error.get().is_some()>
                        <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                            {move || error.get().unwrap_or_default()}
                        </div>
                    </Show>

                    {move || selected.get().map(|detail| {
                        let product_title = detail
                            .translations
                            .first()
                            .map(|item| item.title.clone())
                            .unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "pricing.detail.untitled", "Untitled"));
                        let product_handle = detail
                            .translations
                            .first()
                            .map(|item| item.handle.clone())
                            .unwrap_or_else(|| "-".to_string());
                        let summary = summarize_pricing(detail.variants.as_slice());
                        let shipping_profile = detail
                            .shipping_profile_slug
                            .clone()
                            .unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "pricing.common.unassigned", "unassigned"));
                        let vendor = detail
                            .vendor
                            .clone()
                            .unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "pricing.common.notSet", "not set"));
                        let product_type = detail
                            .product_type
                            .clone()
                            .unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "pricing.common.notSet", "not set"));
                        let status_label = localized_product_status(ui_locale_for_detail.as_deref(), detail.status.as_str());
                        view! {
                            <div class="space-y-6">
                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <div class="flex flex-wrap items-start justify-between gap-3">
                                        <div class="space-y-2">
                                            <div class="flex flex-wrap items-center gap-2">
                                                <h4 class="text-base font-semibold text-card-foreground">{product_title}</h4>
                                                <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", status_badge(detail.status.as_str()))>{status_label}</span>
                                            </div>
                                            <p class="text-sm text-muted-foreground">{format!("handle: {product_handle} | vendor: {vendor} | type: {product_type}")}</p>
                                            <p class="text-xs text-muted-foreground">{format!("shipping profile: {shipping_profile} | updated {}", detail.updated_at)}</p>
                                        </div>
                                        <div class="text-right text-xs text-muted-foreground">
                                            <p>{format!("created {}", detail.created_at)}</p>
                                            <p>{format!("published {}", detail.published_at.unwrap_or_else(|| "-".to_string()))}</p>
                                        </div>
                                    </div>
                                </div>

                                <div class="grid gap-4 md:grid-cols-4">
                                    <StatCard
                                        title=t(ui_locale_for_detail.as_deref(), "pricing.stat.variants", "Variants")
                                        value=summary.variant_count.to_string()
                                        hint=t(ui_locale_for_detail.as_deref(), "pricing.stat.variantsHint", "Tracked SKUs in the selected product.")
                                    />
                                    <StatCard
                                        title=t(ui_locale_for_detail.as_deref(), "pricing.stat.priced", "Priced")
                                        value=summary.priced_variants.to_string()
                                        hint=t(ui_locale_for_detail.as_deref(), "pricing.stat.pricedHint", "Variants that already have at least one configured price.")
                                    />
                                    <StatCard
                                        title=t(ui_locale_for_detail.as_deref(), "pricing.stat.onSale", "On sale")
                                        value=summary.on_sale_variants.to_string()
                                        hint=t(ui_locale_for_detail.as_deref(), "pricing.stat.onSaleHint", "Variants with at least one sale-marked price.")
                                    />
                                    <StatCard
                                        title=t(ui_locale_for_detail.as_deref(), "pricing.stat.currencies", "Currencies")
                                        value=summary.currency_count.to_string()
                                        hint=t(ui_locale_for_detail.as_deref(), "pricing.stat.currenciesHint", "Distinct currency codes across the selected product.")
                                    />
                                </div>

                                <div class="rounded-2xl border border-border bg-background p-5 text-sm text-muted-foreground">
                                    {t(ui_locale_for_detail.as_deref(), "pricing.detail.transportGap", "Dedicated pricing mutations are not split out yet. This route owns price visibility and operator inspection, while price-changing transport still remains in the umbrella ecommerce backlog.")}
                                </div>

                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <div class="flex items-center justify-between gap-3">
                                        <h4 class="text-base font-semibold text-card-foreground">
                                            {t(ui_locale_for_detail.as_deref(), "pricing.section.variants", "Variant prices")}
                                        </h4>
                                        <span class="text-xs text-muted-foreground">
                                            {format!("{} items", detail.variants.len())}
                                        </span>
                                    </div>
                                    <div class="mt-4 space-y-3">
                                        {detail.variants.into_iter().map(|variant| {
                                            let variant_locale = ui_locale_for_variants.clone();
                                            let health_label = pricing_health_label(variant_locale.as_deref(), &variant);
                                            let price_table = format_variant_prices(variant_locale.as_deref(), variant.prices.as_slice());
                                            let identity_label = format_variant_identity(variant_locale.as_deref(), &variant);
                                            let profile_label = variant
                                                .shipping_profile_slug
                                                .clone()
                                                .unwrap_or_else(|| t(variant_locale.as_deref(), "pricing.common.inheritProductProfile", "inherits product profile"));
                                            view! {
                                                <article class="rounded-xl border border-border p-4">
                                                    <div class="flex flex-wrap items-start justify-between gap-3">
                                                        <div class="space-y-2">
                                                            <div class="flex flex-wrap items-center gap-2">
                                                                <h5 class="font-medium text-card-foreground">{variant.title.clone()}</h5>
                                                                <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", pricing_health_badge(&variant))>
                                                                    {health_label}
                                                                </span>
                                                            </div>
                                                            <p class="text-sm text-muted-foreground">{identity_label}</p>
                                                            <p class="text-xs text-muted-foreground">{format!("profile: {profile_label}")}</p>
                                                        </div>
                                                        <div class="space-y-1 text-right text-sm text-muted-foreground">
                                                            <p>{price_table}</p>
                                                        </div>
                                                    </div>
                                                </article>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>
                            </div>
                        }.into_any()
                    }).unwrap_or_else(|| view! {
                        <div class="rounded-2xl border border-dashed border-border p-10 text-center text-sm text-muted-foreground">
                            {t(ui_locale_for_empty.as_deref(), "pricing.detail.empty", "Open a product to inspect variant prices, currency coverage and sale markers from the pricing route.")}
                        </div>
                    }.into_any())}
                </section>
            </div>
        </section>
    }
}

#[component]
fn StatCard(title: String, value: String, hint: String) -> impl IntoView {
    view! {
        <div class="rounded-2xl border border-border bg-background p-4">
            <p class="text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground">{title}</p>
            <p class="mt-3 text-2xl font-semibold text-card-foreground">{value}</p>
            <p class="mt-2 text-xs text-muted-foreground">{hint}</p>
        </div>
    }
}

#[derive(Clone)]
struct PricingSummary {
    variant_count: usize,
    priced_variants: usize,
    on_sale_variants: usize,
    currency_count: usize,
}

fn summarize_pricing(variants: &[PricingVariant]) -> PricingSummary {
    let priced_variants = variants
        .iter()
        .filter(|variant| !variant.prices.is_empty())
        .count();
    let on_sale_variants = variants
        .iter()
        .filter(|variant| variant.prices.iter().any(|price| price.on_sale))
        .count();
    let currency_count = variants
        .iter()
        .flat_map(|variant| {
            variant
                .prices
                .iter()
                .map(|price| price.currency_code.clone())
        })
        .collect::<BTreeSet<_>>()
        .len();

    PricingSummary {
        variant_count: variants.len(),
        priced_variants,
        on_sale_variants,
        currency_count,
    }
}

fn localized_product_status(locale: Option<&str>, status: &str) -> String {
    match status {
        "ACTIVE" => t(locale, "pricing.status.active", "Active"),
        "ARCHIVED" => t(locale, "pricing.status.archived", "Archived"),
        _ => t(locale, "pricing.status.draft", "Draft"),
    }
}

fn format_product_meta(locale: Option<&str>, product: &PricingProductListItem) -> String {
    let vendor = product
        .vendor
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set"));
    let product_type = product
        .product_type
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set"));
    format!(
        "handle: {} | vendor: {} | type: {}",
        product.handle, vendor, product_type
    )
}

fn format_variant_identity(locale: Option<&str>, variant: &PricingVariant) -> String {
    let sku = variant
        .sku
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set"));
    let barcode = variant
        .barcode
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set"));
    format!("sku: {sku} | barcode: {barcode}")
}

fn format_variant_prices(locale: Option<&str>, prices: &[PricingPrice]) -> String {
    if prices.is_empty() {
        return t(locale, "pricing.common.noPricing", "no pricing");
    }

    prices
        .iter()
        .map(|price| match price.compare_at_amount.as_deref() {
            Some(compare_at) if !compare_at.is_empty() => {
                format!(
                    "{} {} (compare-at {})",
                    price.currency_code, price.amount, compare_at
                )
            }
            _ => format!("{} {}", price.currency_code, price.amount),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn pricing_health_label(locale: Option<&str>, variant: &PricingVariant) -> String {
    if variant.prices.is_empty() {
        t(locale, "pricing.health.missing", "No pricing")
    } else if variant.prices.iter().any(|price| price.on_sale) {
        t(locale, "pricing.health.sale", "On sale")
    } else {
        t(locale, "pricing.health.base", "Base price")
    }
}

fn pricing_health_badge(variant: &PricingVariant) -> &'static str {
    if variant.prices.is_empty() {
        "border-rose-200 bg-rose-50 text-rose-700"
    } else if variant.prices.iter().any(|price| price.on_sale) {
        "border-amber-200 bg-amber-50 text-amber-700"
    } else {
        "border-emerald-200 bg-emerald-50 text-emerald-700"
    }
}

fn text_or_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn status_badge(status: &str) -> &'static str {
    match status {
        "ACTIVE" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "ARCHIVED" => "border-slate-200 bg-slate-100 text-slate-700",
        _ => "border-amber-200 bg-amber-50 text-amber-700",
    }
}
