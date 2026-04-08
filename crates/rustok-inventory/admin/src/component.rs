use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{
    InventoryAdminBootstrap, InventoryProductDetail, InventoryProductListItem, InventoryVariant,
};

const LOW_STOCK_THRESHOLD: i32 = 5;

#[component]
pub fn InventoryAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let initial_locale = ui_locale.clone().unwrap_or_else(|| "en".to_string());
    let token = use_token();
    let tenant = use_tenant();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (selected_id, set_selected_id) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<InventoryProductDetail>::None);
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
        "inventory.error.bootstrapLoading",
        "Bootstrap is still loading.",
    );
    let load_product_error_label = t(
        ui_locale.as_deref(),
        "inventory.error.loadProduct",
        "Failed to load inventory detail",
    );
    let product_not_found_label = t(
        ui_locale.as_deref(),
        "inventory.error.productNotFound",
        "Product not found.",
    );
    let load_products_error_label = t(
        ui_locale.as_deref(),
        "inventory.error.loadProducts",
        "Failed to load inventory feed",
    );

    let open_bootstrap_loading_label = bootstrap_loading_label.clone();
    let open_load_product_error_label = load_product_error_label.clone();
    let open_product_not_found_label = product_not_found_label.clone();
    let open_product = Callback::new(move |product_id: String| {
        let Some(InventoryAdminBootstrap { current_tenant }) =
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
                        {t(ui_locale.as_deref(), "inventory.badge", "inventory")}
                    </span>
                    <h2 class="text-2xl font-semibold text-card-foreground">
                        {t(ui_locale.as_deref(), "inventory.title", "Inventory Control")}
                    </h2>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        {t(ui_locale.as_deref(), "inventory.subtitle", "Module-owned inventory read-side surface for stock visibility, low-stock triage and variant health signals while dedicated inventory mutations are still being split from the umbrella transport.")}
                    </p>
                </div>
            </header>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,0.95fr)_minmax(0,1.15fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">
                                {t(ui_locale.as_deref(), "inventory.list.title", "Inventory Feed")}
                            </h3>
                            <p class="text-sm text-muted-foreground">
                                {t(ui_locale.as_deref(), "inventory.list.subtitle", "Search the catalog and open a product to inspect variant-level stock data owned by the inventory boundary.")}
                            </p>
                        </div>
                        <div class="grid gap-3 md:grid-cols-[minmax(0,1fr)_180px_auto]">
                            <input
                                class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                placeholder=t(ui_locale.as_deref(), "inventory.list.search", "Search title")
                                prop:value=move || search.get()
                                on:input=move |ev| set_search.set(event_target_value(&ev))
                            />
                            <select
                                class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                prop:value=move || status_filter.get()
                                on:change=move |ev| set_status_filter.set(event_target_value(&ev))
                            >
                                <option value="">{t(ui_locale.as_deref(), "inventory.filter.allStatuses", "All statuses")}</option>
                                <option value="DRAFT">{t(ui_locale.as_deref(), "inventory.status.draft", "Draft")}</option>
                                <option value="ACTIVE">{t(ui_locale.as_deref(), "inventory.status.active", "Active")}</option>
                                <option value="ARCHIVED">{t(ui_locale.as_deref(), "inventory.status.archived", "Archived")}</option>
                            </select>
                            <button
                                type="button"
                                class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                disabled=move || busy.get()
                                on:click=move |_| set_refresh_nonce.update(|value| *value += 1)
                            >
                                {t(ui_locale.as_deref(), "inventory.action.refresh", "Refresh")}
                            </button>
                        </div>
                    </div>

                    <div class="mt-5 space-y-3">
                        {move || match products.get() {
                            None => view! {
                                <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                                    {t(ui_locale_for_list.as_deref(), "inventory.loading", "Loading inventory feed...")}
                                </div>
                            }.into_any(),
                            Some(Err(err)) => view! {
                                <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                    {format!("{load_products_error_label}: {err}")}
                                </div>
                            }.into_any(),
                            Some(Ok(list)) if list.items.is_empty() => view! {
                                <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                                    {t(ui_locale_for_list.as_deref(), "inventory.list.empty", "No products match the current filters.")}
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
                                            .unwrap_or_else(|| t(item_locale_for_profile.as_deref(), "inventory.common.unassigned", "unassigned"));
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
                                                        {t(item_locale.as_deref(), "inventory.action.open", "Open")}
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
                            {t(ui_locale.as_deref(), "inventory.detail.title", "Inventory Detail")}
                        </h3>
                        <p class="text-sm text-muted-foreground">
                            {t(ui_locale.as_deref(), "inventory.detail.subtitle", "Inspect stock health, backorder policy and per-variant visibility from the inventory-owned route.")}
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
                            .unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "inventory.detail.untitled", "Untitled"));
                        let product_handle = detail
                            .translations
                            .first()
                            .map(|item| item.handle.clone())
                            .unwrap_or_else(|| "-".to_string());
                        let summary = summarize_inventory(detail.variants.as_slice());
                        let shipping_profile = detail
                            .shipping_profile_slug
                            .clone()
                            .unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "inventory.common.unassigned", "unassigned"));
                        let vendor = detail
                            .vendor
                            .clone()
                            .unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "inventory.common.notSet", "not set"));
                        let product_type = detail
                            .product_type
                            .clone()
                            .unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "inventory.common.notSet", "not set"));
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
                                        title=t(ui_locale_for_detail.as_deref(), "inventory.stat.variants", "Variants")
                                        value=summary.variant_count.to_string()
                                        hint=t(ui_locale_for_detail.as_deref(), "inventory.stat.variantsHint", "Tracked SKUs in the selected product.")
                                    />
                                    <StatCard
                                        title=t(ui_locale_for_detail.as_deref(), "inventory.stat.totalQuantity", "On-hand")
                                        value=summary.total_quantity.to_string()
                                        hint=t(ui_locale_for_detail.as_deref(), "inventory.stat.totalQuantityHint", "Summed quantity from the current product contract.")
                                    />
                                    <StatCard
                                        title=t(ui_locale_for_detail.as_deref(), "inventory.stat.lowStock", "Low stock")
                                        value=summary.low_stock.to_string()
                                        hint=t(ui_locale_for_detail.as_deref(), "inventory.stat.lowStockHint", "Variants at or below the low-stock threshold.")
                                    />
                                    <StatCard
                                        title=t(ui_locale_for_detail.as_deref(), "inventory.stat.backorder", "Backorder")
                                        value=summary.backorder.to_string()
                                        hint=t(ui_locale_for_detail.as_deref(), "inventory.stat.backorderHint", "Variants that continue selling below zero.")
                                    />
                                </div>

                                <div class="rounded-2xl border border-border bg-background p-5 text-sm text-muted-foreground">
                                    {t(ui_locale_for_detail.as_deref(), "inventory.detail.transportGap", "Dedicated inventory mutations are not split out yet. This route owns stock visibility and operator triage, while quantity-changing transport remains to be extracted from the umbrella ecommerce surface.")}
                                </div>

                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <div class="flex items-center justify-between gap-3">
                                        <h4 class="text-base font-semibold text-card-foreground">
                                            {t(ui_locale_for_detail.as_deref(), "inventory.section.variants", "Variant stock")}
                                        </h4>
                                        <span class="text-xs text-muted-foreground">
                                            {format!("{} items", detail.variants.len())}
                                        </span>
                                    </div>
                                    <div class="mt-4 space-y-3">
                                        {detail.variants.into_iter().map(|variant| {
                                            let variant_locale = ui_locale_for_variants.clone();
                                            let health_label = inventory_health_label(variant_locale.as_deref(), &variant);
                                            let price_label = format_variant_price(variant_locale.as_deref(), &variant);
                                            let identity_label = format_variant_identity(variant_locale.as_deref(), &variant);
                                            let profile_label = variant
                                                .shipping_profile_slug
                                                .clone()
                                                .unwrap_or_else(|| t(variant_locale.as_deref(), "inventory.common.inheritProductProfile", "inherits product profile"));
                                            view! {
                                                <article class="rounded-xl border border-border p-4">
                                                    <div class="flex flex-wrap items-start justify-between gap-3">
                                                        <div class="space-y-2">
                                                            <div class="flex flex-wrap items-center gap-2">
                                                                <h5 class="font-medium text-card-foreground">{variant.title.clone()}</h5>
                                                                <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", inventory_health_badge(&variant))>
                                                                    {health_label}
                                                                </span>
                                                            </div>
                                                            <p class="text-sm text-muted-foreground">{identity_label}</p>
                                                            <p class="text-xs text-muted-foreground">{format!("profile: {profile_label}")}</p>
                                                        </div>
                                                        <div class="space-y-1 text-right text-sm text-muted-foreground">
                                                            <p>{format!("qty {}", variant.inventory_quantity)}</p>
                                                            <p>{format!("policy {}", variant.inventory_policy)}</p>
                                                            <p>{format!("stock {}", bool_label(variant_locale.as_deref(), variant.in_stock))}</p>
                                                            <p class="text-xs">{price_label}</p>
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
                            {t(ui_locale_for_empty.as_deref(), "inventory.detail.empty", "Open a product to inspect variant stock, low-stock signals and backorder policy from the inventory route.")}
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

#[derive(Clone, Copy)]
struct InventorySummary {
    variant_count: usize,
    total_quantity: i32,
    low_stock: usize,
    backorder: usize,
}

fn summarize_inventory(variants: &[InventoryVariant]) -> InventorySummary {
    InventorySummary {
        variant_count: variants.len(),
        total_quantity: variants
            .iter()
            .map(|variant| variant.inventory_quantity)
            .sum(),
        low_stock: variants
            .iter()
            .filter(|variant| variant.inventory_quantity <= LOW_STOCK_THRESHOLD)
            .count(),
        backorder: variants
            .iter()
            .filter(|variant| variant.inventory_policy.eq_ignore_ascii_case("continue"))
            .count(),
    }
}

fn localized_product_status(locale: Option<&str>, status: &str) -> String {
    match status {
        "ACTIVE" => t(locale, "inventory.status.active", "Active"),
        "ARCHIVED" => t(locale, "inventory.status.archived", "Archived"),
        _ => t(locale, "inventory.status.draft", "Draft"),
    }
}

fn format_product_meta(locale: Option<&str>, product: &InventoryProductListItem) -> String {
    let vendor = product
        .vendor
        .clone()
        .unwrap_or_else(|| t(locale, "inventory.common.notSet", "not set"));
    let product_type = product
        .product_type
        .clone()
        .unwrap_or_else(|| t(locale, "inventory.common.notSet", "not set"));
    format!(
        "handle: {} | vendor: {} | type: {}",
        product.handle, vendor, product_type
    )
}

fn format_variant_identity(locale: Option<&str>, variant: &InventoryVariant) -> String {
    let sku = variant
        .sku
        .clone()
        .unwrap_or_else(|| t(locale, "inventory.common.notSet", "not set"));
    let barcode = variant
        .barcode
        .clone()
        .unwrap_or_else(|| t(locale, "inventory.common.notSet", "not set"));
    format!("sku: {sku} | barcode: {barcode}")
}

fn format_variant_price(locale: Option<&str>, variant: &InventoryVariant) -> String {
    if variant.prices.is_empty() {
        t(locale, "inventory.common.noPricing", "no pricing")
    } else {
        variant
            .prices
            .iter()
            .map(|price| format!("{} {}", price.currency_code, price.amount))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn inventory_health_label(locale: Option<&str>, variant: &InventoryVariant) -> String {
    if !variant.in_stock {
        t(locale, "inventory.health.outOfStock", "Out of stock")
    } else if variant.inventory_policy.eq_ignore_ascii_case("continue") {
        t(locale, "inventory.health.backorder", "Backorder")
    } else if variant.inventory_quantity <= LOW_STOCK_THRESHOLD {
        t(locale, "inventory.health.lowStock", "Low stock")
    } else {
        t(locale, "inventory.health.healthy", "Healthy")
    }
}

fn inventory_health_badge(variant: &InventoryVariant) -> &'static str {
    if !variant.in_stock {
        "border-rose-200 bg-rose-50 text-rose-700"
    } else if variant.inventory_policy.eq_ignore_ascii_case("continue") {
        "border-sky-200 bg-sky-50 text-sky-700"
    } else if variant.inventory_quantity <= LOW_STOCK_THRESHOLD {
        "border-amber-200 bg-amber-50 text-amber-700"
    } else {
        "border-emerald-200 bg-emerald-50 text-emerald-700"
    }
}

fn bool_label(locale: Option<&str>, value: bool) -> String {
    if value {
        t(locale, "inventory.bool.yes", "yes")
    } else {
        t(locale, "inventory.bool.no", "no")
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
