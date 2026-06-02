use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{AdminQueryKey, UiRouteContext};

use crate::core::{inventory_health_state, summarize_inventory, InventoryHealthState};
use crate::i18n::t;
use crate::model::{
    InventoryAdminBootstrap, InventoryProductDetail, InventoryProductListItem, InventoryVariant,
};

fn local_resource<S, Fut, T>(
    source: impl Fn() -> S + 'static,
    fetcher: impl Fn(S) -> Fut + 'static,
) -> LocalResource<T>
where
    S: 'static,
    Fut: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    LocalResource::new(move || fetcher(source()))
}

fn locale_tags_match(left: &str, right: &str) -> bool {
    left.trim()
        .replace('_', "-")
        .eq_ignore_ascii_case(&right.trim().replace('_', "-"))
}

#[component]
pub fn InventoryAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let effective_locale = ui_locale.clone();
    let selected_product_query = use_route_query_value(AdminQueryKey::ProductId.as_str());
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (selected_id, set_selected_id) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<InventoryProductDetail>::None);
    let (search, set_search) = signal(String::new());
    let (status_filter, set_status_filter) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let effective_locale_for_products = effective_locale.clone();
    let effective_locale_for_open = effective_locale.clone();

    let bootstrap = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            crate::api::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let products = local_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                effective_locale_for_products.clone(),
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
        let locale_value = effective_locale_for_open.clone();
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
                Ok(None) => {
                    set_selected_id.set(None);
                    set_selected.set(None);
                    set_error.set(Some(not_found_label));
                }
                Err(err) => {
                    set_selected_id.set(None);
                    set_selected.set(None);
                    set_error.set(Some(format!("{load_error_label}: {err}")));
                }
            }
            set_busy.set(false);
        });
    });

    let ui_locale_for_list = ui_locale.clone();
    let ui_locale_for_list_status = ui_locale.clone();
    let ui_locale_for_detail = ui_locale.clone();
    let ui_locale_for_variants = ui_locale.clone();
    let ui_locale_for_empty = ui_locale.clone();
    let effective_locale_for_detail = effective_locale.clone();
    let initial_open_product = open_product;
    let list_query_writer = query_writer.clone();
    Effect::new(move |_| match selected_product_query.get() {
        Some(product_id) if !product_id.trim().is_empty() => {
            if bootstrap.get().and_then(Result::ok).is_none() {
                return;
            }
            initial_open_product.run(product_id);
        }
        _ => {
            set_selected_id.set(None);
            set_selected.set(None);
        }
    });

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
                                        let item_query_writer = list_query_writer.clone();
                                        let item_locale = ui_locale_for_list_status.clone();
                                        let item_locale_for_meta = item_locale.clone();
                                        let item_locale_for_profile = item_locale.clone();
                                        let shipping_profile = product.shipping_profile_slug.clone();
                                        let profile_label = shipping_profile
                                            .unwrap_or_else(|| t(item_locale_for_profile.as_deref(), "inventory.common.unassigned", "unassigned"));
                                        view! {
                                            <article class=move || {
                                                if selected_id.get().as_deref() == Some(selected_marker.as_str()) {
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
                                                        on:click=move |_| item_query_writer.push_value(AdminQueryKey::ProductId.as_str(), open_id.clone())
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
                        let resolved_translation = inventory_translation_for_locale(
                            detail.translations.as_slice(),
                            effective_locale_for_detail.as_deref(),
                        );
                        let product_title = resolved_translation
                            .map(|item| item.title.clone())
                            .unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "inventory.detail.untitled", "Untitled"));
                        let product_handle = resolved_translation
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

                                <div class="grid gap-4 md:grid-cols-6">
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
                                    <StatCard
                                        title=t(ui_locale_for_detail.as_deref(), "inventory.stat.outOfStock", "Out of stock")
                                        value=summary.out_of_stock.to_string()
                                        hint=t(ui_locale_for_detail.as_deref(), "inventory.stat.outOfStockHint", "Variants currently unavailable for immediate sale.")
                                    />
                                    <StatCard
                                        title=t(ui_locale_for_detail.as_deref(), "inventory.stat.healthy", "Healthy")
                                        value=summary.healthy.to_string()
                                        hint=t(ui_locale_for_detail.as_deref(), "inventory.stat.healthyHint", "Variants with sufficient stock and no backorder handling.")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        summarize_inventory_health_counts, InventoryHealthCounts, LOW_STOCK_THRESHOLD,
    };
    use crate::model::InventoryVariant;

    fn variant(
        in_stock: bool,
        inventory_policy: &str,
        inventory_quantity: i32,
    ) -> InventoryVariant {
        InventoryVariant {
            id: "v".to_string(),
            sku: None,
            barcode: None,
            shipping_profile_slug: None,
            title: "Variant".to_string(),
            option1: None,
            option2: None,
            option3: None,
            prices: Vec::new(),
            inventory_quantity,
            inventory_policy: inventory_policy.to_string(),
            in_stock,
        }
    }

    fn healthy_count(variants: &[InventoryVariant]) -> usize {
        variants
            .iter()
            .filter(|variant| inventory_health_state(variant) == InventoryHealthState::Healthy)
            .count()
    }

    #[test]
    fn summary_keeps_low_stock_out_of_stock_and_backorder_disjoint() {
        let variants = vec![
            variant(true, "deny", 2),
            variant(false, "deny", 0),
            variant(true, "continue", 0),
            variant(false, "continue", -3),
        ];

        let summary = summarize_inventory(&variants);
        assert_eq!(summary.variant_count, 4);
        assert_eq!(summary.low_stock, 1);
        assert_eq!(summary.out_of_stock, 1);
        assert_eq!(summary.backorder, 2);
    }

    #[test]
    fn health_label_and_badge_follow_backorder_precedence() {
        let variant = variant(false, "continue", -1);

        let label = inventory_health_label(None, &variant);
        let badge = inventory_health_badge(&variant);

        assert_eq!(label, "Backorder");
        assert_eq!(badge, "border-sky-200 bg-sky-50 text-sky-700");
    }

    #[test]
    fn health_label_and_badge_mark_out_of_stock_when_backorder_disabled() {
        let variant = variant(false, "deny", 0);

        let label = inventory_health_label(None, &variant);
        let badge = inventory_health_badge(&variant);

        assert_eq!(label, "Out of stock");
        assert_eq!(badge, "border-rose-200 bg-rose-50 text-rose-700");
    }

    #[test]
    fn summary_and_health_treat_backorder_policy_case_insensitively() {
        let backorder_upper = variant(false, "CONTINUE", -2);
        let low_stock_regular = variant(true, "deny", LOW_STOCK_THRESHOLD);
        let healthy = variant(true, "deny", LOW_STOCK_THRESHOLD + 1);
        let variants = vec![
            backorder_upper.clone(),
            low_stock_regular.clone(),
            healthy.clone(),
        ];

        let summary = summarize_inventory(&variants);
        assert_eq!(summary.backorder, 1);
        assert_eq!(summary.out_of_stock, 0);
        assert_eq!(summary.low_stock, 1);

        assert_eq!(inventory_health_label(None, &backorder_upper), "Backorder");
        assert_eq!(
            inventory_health_label(None, &low_stock_regular),
            "Low stock"
        );
        assert_eq!(inventory_health_label(None, &healthy), "Healthy");
    }

    #[test]
    fn summary_counts_are_partitioned_by_health_state() {
        let variants = vec![
            variant(true, "deny", LOW_STOCK_THRESHOLD + 5),
            variant(true, "deny", LOW_STOCK_THRESHOLD),
            variant(false, "deny", 0),
            variant(true, "continue", -1),
        ];

        let summary = summarize_inventory(&variants);
        let healthy_count = healthy_count(&variants);
        let covered = summarize_inventory_health_counts(&variants).non_healthy_total();
        assert_eq!(covered, summary.variant_count - healthy_count);
        assert_eq!(
            covered,
            summary.low_stock + summary.out_of_stock + summary.backorder
        );
    }

    #[test]
    fn health_counts_empty_input_is_zeroed() {
        let counts = summarize_inventory_health_counts(&[]);
        assert_eq!(counts, InventoryHealthCounts::default());
    }

    #[test]
    fn summary_empty_input_is_zeroed() {
        let summary = summarize_inventory(&[]);
        assert_eq!(summary.variant_count, 0);
        assert_eq!(summary.total_quantity, 0);
        assert_eq!(summary.low_stock, 0);
        assert_eq!(summary.out_of_stock, 0);
        assert_eq!(summary.backorder, 0);
        assert_eq!(summary.healthy, 0);
    }

    #[test]
    fn health_counts_and_summary_stay_consistent_for_mixed_variants() {
        let variants = vec![
            variant(true, "deny", LOW_STOCK_THRESHOLD - 1),
            variant(true, "deny", LOW_STOCK_THRESHOLD + 3),
            variant(false, "deny", 0),
            variant(false, "continue", -2),
            variant(true, "CONTINUE", -1),
        ];

        let counts = summarize_inventory_health_counts(&variants);
        let summary = summarize_inventory(&variants);
        let healthy_count = healthy_count(&variants);
        assert_eq!(counts.low_stock, summary.low_stock);
        assert_eq!(counts.out_of_stock, summary.out_of_stock);
        assert_eq!(counts.backorder, summary.backorder);
        assert_eq!(
            counts.non_healthy_total(),
            summary.variant_count - healthy_count
        );
    }

    #[test]
    fn summary_total_quantity_is_independent_from_health_partition() {
        let variants = vec![
            variant(true, "deny", 7),
            variant(true, "deny", -2),
            variant(false, "deny", 0),
            variant(false, "continue", -5),
        ];

        let summary = summarize_inventory(&variants);
        assert_eq!(summary.total_quantity, 0);
        assert_eq!(summary.variant_count, 4);
        assert_eq!(
            summary.low_stock + summary.out_of_stock + summary.backorder,
            3
        );
    }

    #[test]
    fn healthy_count_matches_variant_count_minus_non_healthy_total() {
        let variants = vec![
            variant(true, "deny", 12),
            variant(true, "deny", 2),
            variant(false, "deny", 0),
            variant(true, "continue", -3),
            variant(false, "continue", -4),
        ];

        let counts = summarize_inventory_health_counts(&variants);
        let healthy_count = healthy_count(&variants);
        assert_eq!(
            healthy_count,
            variants.len().saturating_sub(counts.non_healthy_total())
        );
        let summary = summarize_inventory(&variants);
        assert_eq!(summary.healthy, healthy_count);
    }

    #[test]
    fn health_state_label_and_badge_are_consistent_for_each_state() {
        let healthy = variant(true, "deny", LOW_STOCK_THRESHOLD + 3);
        let low_stock = variant(true, "deny", LOW_STOCK_THRESHOLD);
        let out_of_stock = variant(false, "deny", 0);
        let backorder = variant(false, "continue", -2);

        assert_eq!(
            inventory_health_state(&healthy),
            InventoryHealthState::Healthy
        );
        assert_eq!(inventory_health_label(None, &healthy), "Healthy");
        assert_eq!(
            inventory_health_badge(&healthy),
            "border-emerald-200 bg-emerald-50 text-emerald-700"
        );

        assert_eq!(
            inventory_health_state(&low_stock),
            InventoryHealthState::LowStock
        );
        assert_eq!(inventory_health_label(None, &low_stock), "Low stock");
        assert_eq!(
            inventory_health_badge(&low_stock),
            "border-amber-200 bg-amber-50 text-amber-700"
        );

        assert_eq!(
            inventory_health_state(&out_of_stock),
            InventoryHealthState::OutOfStock
        );
        assert_eq!(inventory_health_label(None, &out_of_stock), "Out of stock");
        assert_eq!(
            inventory_health_badge(&out_of_stock),
            "border-rose-200 bg-rose-50 text-rose-700"
        );

        assert_eq!(
            inventory_health_state(&backorder),
            InventoryHealthState::Backorder
        );
        assert_eq!(inventory_health_label(None, &backorder), "Backorder");
        assert_eq!(
            inventory_health_badge(&backorder),
            "border-sky-200 bg-sky-50 text-sky-700"
        );
    }

    #[test]
    fn summary_partition_is_complete_including_healthy_bucket() {
        let variants = vec![
            variant(true, "deny", LOW_STOCK_THRESHOLD + 3),
            variant(true, "deny", LOW_STOCK_THRESHOLD),
            variant(false, "deny", 0),
            variant(true, "continue", -1),
        ];

        let summary = summarize_inventory(&variants);
        let partition_total =
            summary.healthy + summary.low_stock + summary.out_of_stock + summary.backorder;
        assert_eq!(partition_total, summary.variant_count);
    }

    #[test]
    fn summary_partition_matches_health_count_helper_totals() {
        let variants = vec![
            variant(true, "deny", LOW_STOCK_THRESHOLD + 10),
            variant(true, "deny", LOW_STOCK_THRESHOLD - 1),
            variant(false, "deny", 0),
            variant(true, "continue", -1),
            variant(false, "continue", -2),
        ];

        let counts = summarize_inventory_health_counts(&variants);
        let summary = summarize_inventory(&variants);

        assert_eq!(summary.low_stock, counts.low_stock);
        assert_eq!(summary.out_of_stock, counts.out_of_stock);
        assert_eq!(summary.backorder, counts.backorder);
        assert_eq!(summary.healthy, variants.len() - counts.non_healthy_total());
        assert_eq!(
            summary.healthy + counts.non_healthy_total(),
            summary.variant_count
        );
    }

    #[test]
    fn summary_healthy_bucket_matches_healthy_state_projection() {
        let variants = vec![
            variant(true, "deny", LOW_STOCK_THRESHOLD + 2),
            variant(true, "deny", LOW_STOCK_THRESHOLD + 20),
            variant(true, "deny", LOW_STOCK_THRESHOLD),
            variant(false, "deny", 0),
            variant(true, "continue", -1),
        ];

        let summary = summarize_inventory(&variants);
        let projected_healthy = variants
            .iter()
            .filter(|variant| inventory_health_state(variant) == InventoryHealthState::Healthy)
            .count();
        assert_eq!(summary.healthy, projected_healthy);
    }

    #[test]
    fn summary_variant_count_matches_input_length() {
        let variants = vec![
            variant(true, "deny", LOW_STOCK_THRESHOLD + 1),
            variant(true, "deny", LOW_STOCK_THRESHOLD),
            variant(false, "deny", 0),
            variant(true, "continue", -1),
            variant(false, "continue", -2),
            variant(true, "deny", 42),
        ];

        let summary = summarize_inventory(&variants);
        assert_eq!(summary.variant_count, variants.len());
    }

    #[test]
    fn state_label_helper_matches_variant_label_projection() {
        let variants = vec![
            variant(true, "deny", LOW_STOCK_THRESHOLD + 3),
            variant(true, "deny", LOW_STOCK_THRESHOLD),
            variant(false, "deny", 0),
            variant(false, "continue", -2),
        ];

        for variant in variants {
            let state = inventory_health_state(&variant);
            assert_eq!(
                inventory_health_label_for_state(None, state),
                inventory_health_label(None, &variant)
            );
        }
    }

    #[test]
    fn state_badge_helper_matches_variant_badge_projection() {
        let variants = vec![
            variant(true, "deny", LOW_STOCK_THRESHOLD + 3),
            variant(true, "deny", LOW_STOCK_THRESHOLD),
            variant(false, "deny", 0),
            variant(false, "continue", -2),
        ];

        for variant in variants {
            let state = inventory_health_state(&variant);
            assert_eq!(
                inventory_health_badge_for_state(state),
                inventory_health_badge(&variant)
            );
        }
    }
}

fn inventory_translation_for_locale<'a>(
    translations: &'a [crate::model::InventoryProductTranslation],
    requested_locale: Option<&str>,
) -> Option<&'a crate::model::InventoryProductTranslation> {
    requested_locale
        .and_then(|requested_locale| {
            translations
                .iter()
                .find(|translation| locale_tags_match(&translation.locale, requested_locale))
        })
        .or_else(|| translations.first())
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
    inventory_health_label_for_state(locale, inventory_health_state(variant))
}

fn inventory_health_badge(variant: &InventoryVariant) -> &'static str {
    inventory_health_badge_for_state(inventory_health_state(variant))
}

fn inventory_health_label_for_state(locale: Option<&str>, state: InventoryHealthState) -> String {
    match state {
        InventoryHealthState::Backorder => t(locale, "inventory.health.backorder", "Backorder"),
        InventoryHealthState::OutOfStock => {
            t(locale, "inventory.health.outOfStock", "Out of stock")
        }
        InventoryHealthState::LowStock => t(locale, "inventory.health.lowStock", "Low stock"),
        InventoryHealthState::Healthy => t(locale, "inventory.health.healthy", "Healthy"),
    }
}

fn inventory_health_badge_for_state(state: InventoryHealthState) -> &'static str {
    match state {
        InventoryHealthState::Backorder => "border-sky-200 bg-sky-50 text-sky-700",
        InventoryHealthState::OutOfStock => "border-rose-200 bg-rose-50 text-rose-700",
        InventoryHealthState::LowStock => "border-amber-200 bg-amber-50 text-amber-700",
        InventoryHealthState::Healthy => "border-emerald-200 bg-emerald-50 text-emerald-700",
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
