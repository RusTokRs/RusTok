use super::detail::PricingDetailSection;
use crate::core::{
    build_resolution_context, filter_pricing_products, normalize_channel_value,
    normalized_currency_code, normalized_price_list_id, normalized_quantity, normalized_region_id,
    pricing_grid_columns, text_or_none,
};
use crate::i18n::t;
use crate::model::{
    PricingAdminBootstrap, PricingProductDetail, PricingProductListItem, PricingResolutionContext,
};
use crate::transport;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_auth_ui::ui::leptos::hooks::{use_tenant, use_token};
use rustok_grid::{ColumnFilters, GridPagination, RowSelection};
use rustok_grid_leptos::DataGrid;
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

/// Builds a resource the response resolves.
///
/// The module admin mounts inside the layout route that declares `SsrMode::Async`, so a
/// `LocalResource` here is `pending()` on the server and the reader is served the `Suspense`
/// fallback. See `DECISIONS/2026-11-13-standalone-admin-editor-delivery.md`.
fn loaded_resource<S, Fut, T>(
    source: impl Fn() -> S + Send + Sync + 'static,
    fetcher: impl Fn(S) -> Fut + Send + Sync + 'static,
) -> Resource<T>
where
    S: PartialEq + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = T> + Send + 'static,
    T: Send + Sync + 'static,
{
    Resource::new(source, fetcher)
}

#[component]
pub fn PricingAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let effective_locale = ui_locale.clone();
    let selected_product_query = use_route_query_value(AdminQueryKey::ProductId.as_str());
    let currency_query = use_route_query_value(AdminQueryKey::Currency.as_str());
    let region_id_query = use_route_query_value(AdminQueryKey::RegionId.as_str());
    let price_list_id_query = use_route_query_value(AdminQueryKey::PriceListId.as_str());
    let channel_id_query = use_route_query_value(AdminQueryKey::ChannelId.as_str());
    let channel_slug_query = use_route_query_value(AdminQueryKey::ChannelSlug.as_str());
    let quantity_query = use_route_query_value(AdminQueryKey::Quantity.as_str());
    let query_writer = use_route_query_writer();
    let initial_currency = currency_query.get_untracked().unwrap_or_default();
    let initial_region_id = region_id_query.get_untracked().unwrap_or_default();
    let initial_price_list_id = price_list_id_query.get_untracked().unwrap_or_default();
    let initial_channel_id = channel_id_query.get_untracked().unwrap_or_default();
    let initial_channel_slug = channel_slug_query.get_untracked().unwrap_or_default();
    let initial_quantity = quantity_query
        .get_untracked()
        .unwrap_or_else(|| "1".to_string());
    let token = use_token();
    let tenant = use_tenant();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (selected_id, set_selected_id) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<PricingProductDetail>::None);
    let (search, set_search) = signal(String::new());
    let (status_filter, set_status_filter) = signal(String::new());
    let (resolution_currency, set_resolution_currency) = signal(initial_currency);
    let (resolution_region_id, set_resolution_region_id) = signal(initial_region_id);
    let (resolution_price_list_id, set_resolution_price_list_id) = signal(initial_price_list_id);
    let (resolution_channel_id, set_resolution_channel_id) = signal(initial_channel_id);
    let (resolution_channel_slug, set_resolution_channel_slug) = signal(initial_channel_slug);
    let (resolution_quantity, set_resolution_quantity) = signal(initial_quantity);
    let (applied_resolution_context, set_applied_resolution_context) =
        signal(Option::<PricingResolutionContext>::None);
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let effective_locale_for_products = effective_locale.clone();
    let effective_locale_for_open = effective_locale.clone();

    let bootstrap = loaded_resource(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            transport::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let products = loaded_resource(
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
                transport::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            transport::fetch_products(
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

    let context_active_price_lists = loaded_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                resolution_channel_id.get(),
                resolution_channel_slug.get(),
            )
        },
        move |(token_value, tenant_value, _, channel_id, channel_slug)| async move {
            transport::fetch_active_price_lists(
                token_value,
                tenant_value,
                normalize_channel_value(&channel_id),
                normalize_channel_value(&channel_slug),
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
        let Some(PricingAdminBootstrap { current_tenant, .. }) =
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
        let currency_value = resolution_currency.get_untracked();
        let region_value = resolution_region_id.get_untracked();
        let price_list_value = resolution_price_list_id.get_untracked();
        let channel_id_value = resolution_channel_id.get_untracked();
        let channel_slug_value = resolution_channel_slug.get_untracked();
        let quantity_value = resolution_quantity.get_untracked();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let next_context = build_resolution_context(
                currency_value.clone(),
                region_value.clone(),
                price_list_value.clone(),
                channel_id_value.clone(),
                channel_slug_value.clone(),
                quantity_value.clone(),
            );
            match transport::fetch_product(
                token_value,
                tenant_value,
                current_tenant.id,
                product_id,
                locale_value,
                normalized_currency_code(currency_value),
                normalized_region_id(region_value),
                normalized_price_list_id(price_list_value),
                normalize_channel_value(&channel_id_value),
                normalize_channel_value(&channel_slug_value),
                normalized_quantity(quantity_value),
            )
            .await
            {
                Ok(Some(product)) => {
                    set_selected_id.set(Some(product.id.clone()));
                    set_selected.set(Some(product));
                    set_applied_resolution_context.set(next_context);
                }
                Ok(None) => {
                    set_selected_id.set(None);
                    set_selected.set(None);
                    set_error.set(Some(not_found_label));
                    set_applied_resolution_context.set(None);
                }
                Err(err) => {
                    set_selected_id.set(None);
                    set_selected.set(None);
                    set_applied_resolution_context.set(None);
                    set_error.set(Some(format!("{load_error_label}: {err}")));
                }
            }
            set_busy.set(false);
        });
    });

    let ui_locale_for_list = ui_locale.clone();

    let product_module_route_base = route_context.module_route_base("product");
    let refresh_open_product = open_product;
    let initial_open_product = open_product;
    let list_query_writer = query_writer.clone();
    let context_query_writer = query_writer.clone();

    let refresh_detail = Callback::new(move |_| {
        set_refresh_nonce.update(|value| *value += 1);
        if let Some(product_id) = selected_id.get_untracked() {
            refresh_open_product.run(product_id);
        }
    });
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
            set_applied_resolution_context.set(None);
        }
    });
    Effect::new(move |_| {
        set_resolution_currency.set(currency_query.get().unwrap_or_default());
        set_resolution_region_id.set(region_id_query.get().unwrap_or_default());
        set_resolution_price_list_id.set(price_list_id_query.get().unwrap_or_default());
        set_resolution_channel_id.set(channel_id_query.get().unwrap_or_default());
        set_resolution_channel_slug.set(channel_slug_query.get().unwrap_or_default());
        set_resolution_quantity.set(quantity_query.get().unwrap_or_else(|| "1".to_string()));
    });

    let is_ru = ui_locale.as_deref() == Some("ru");
    let columns = pricing_grid_columns(ui_locale.as_deref());
    let filters = RwSignal::new(ColumnFilters::new());
    let selection = RwSignal::new(RowSelection::new());
    let pagination = RwSignal::new(GridPagination::new(1, 10, 0));

    let filtered_products = Memo::new(move |_| {
        let raw = products
            .get()
            .and_then(Result::ok)
            .map(|l| l.items)
            .unwrap_or_default();
        let current_filters = filters.get();
        filter_pricing_products(&raw, &current_filters)
    });

    let on_filters_change = Callback::new(move |new_filters: ColumnFilters| {
        filters.set(new_filters);
    });

    let cell_locale = ui_locale.clone();
    let cell_selected_id = selected_id;
    let cell_action_writer = list_query_writer.clone();
    let cell_open_label = t(ui_locale.as_deref(), "pricing.action.open", "Open");
    let cell_renderer = Callback::new(move |(item, col_id): (PricingProductListItem, String)| {
        match col_id.as_str() {
            "title" => {
                let is_sel = cell_selected_id.get().as_deref() == Some(&item.id);
                view! {
                    <div class="flex flex-col">
                        <span class=if is_sel {
                            "text-xs font-semibold text-primary underline"
                        } else {
                            "text-xs font-medium text-foreground hover:text-primary transition"
                        }>
                            {item.title.clone()}
                        </span>
                        <span class="text-[11px] text-muted-foreground/80 font-mono">
                            {item.handle.clone()}
                        </span>
                    </div>
                }
                .into_any()
            }
            "status" => {
                let badge_cls = match item.status.as_str() {
                    "ACTIVE" => "border-emerald-200 bg-emerald-50 text-emerald-700",
                    "ARCHIVED" => "border-slate-200 bg-slate-100 text-slate-700",
                    _ => "border-amber-200 bg-amber-50 text-amber-700",
                };
                let label = match item.status.as_str() {
                    "ACTIVE" => t(cell_locale.as_deref(), "pricing.status.active", "Active"),
                    "ARCHIVED" => t(
                        cell_locale.as_deref(),
                        "pricing.status.archived",
                        "Archived",
                    ),
                    _ => t(cell_locale.as_deref(), "pricing.status.draft", "Draft"),
                };
                view! {
                    <span class=format!("inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-semibold border {badge_cls}")>
                        {label}
                    </span>
                }
                .into_any()
            }
            "product_type" => {
                let p_type = item.product_type.clone().unwrap_or_else(|| "—".to_string());
                view! {
                    <span class="text-xs text-muted-foreground truncate">
                        {p_type}
                    </span>
                }
                .into_any()
            }
            "vendor" => {
                let vendor_str = item.vendor.clone().unwrap_or_else(|| "—".to_string());
                view! {
                    <span class="text-xs text-muted-foreground truncate">
                        {vendor_str}
                    </span>
                }
                .into_any()
            }
            "shipping_profile_slug" => {
                let profile_str = item
                    .shipping_profile_slug
                    .clone()
                    .unwrap_or_else(|| "—".to_string());
                view! {
                    <span class="text-xs text-muted-foreground truncate">
                        {profile_str}
                    </span>
                }
                .into_any()
            }
            "created_at" => {
                let date_str = item
                    .created_at
                    .split('T')
                    .next()
                    .unwrap_or(&item.created_at);
                view! {
                    <span class="text-xs text-muted-foreground whitespace-nowrap">
                        {date_str.to_string()}
                    </span>
                }
                .into_any()
            }
            "actions" => {
                let open_id = item.id.clone();
                let item_writer = cell_action_writer.clone();
                let btn_label = cell_open_label.clone();
                view! {
                    <div class="flex items-center justify-center">
                        <button
                            type="button"
                            class="inline-flex items-center justify-center h-6 px-2.5 rounded-md text-[11px] font-medium bg-secondary text-secondary-foreground hover:bg-accent transition"
                            on:click=move |ev| {
                                ev.stop_propagation();
                                item_writer.push_value(AdminQueryKey::ProductId.as_str(), open_id.clone());
                            }
                        >
                            {btn_label}
                        </button>
                    </div>
                }
                .into_any()
            }
            _ => ().into_any(),
        }
    });

    let on_row_click = {
        let click_writer = query_writer.clone();
        Callback::new(move |item: PricingProductListItem| {
            click_writer.push_value(AdminQueryKey::ProductId.as_str(), item.id);
        })
    };

    let product_error = Memo::new(move |_| {
        products
            .get()
            .and_then(|res| res.err().map(|e| e.to_string()))
    });
    let load_error_prefix = StoredValue::new(load_products_error_label);

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

                <Show when=move || product_error.get().is_some()>
                    <div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                        {move || format!("{}: {}", load_error_prefix.get_value(), product_error.get().unwrap_or_default())}
                    </div>
                </Show>

                // Selection toolbar when items selected
                <Show when=move || !selection.get().is_empty()>
                    <div class="mt-4 flex items-center justify-between gap-3 bg-primary/5 border border-primary/20 rounded-xl px-4 py-2 animate-in fade-in duration-150">
                        <div class="flex items-center gap-2">
                            <span class="w-2 h-2 rounded-full bg-primary animate-pulse" />
                            <span class="text-xs font-semibold text-foreground">
                                {move || format!("{} {}", selection.get().count(), if is_ru { "выбрано" } else { "selected" })}
                            </span>
                        </div>
                        <button
                            type="button"
                            class="h-6 px-2.5 rounded-lg text-xs text-muted-foreground hover:text-foreground transition border border-border bg-background"
                            on:click=move |_| selection.update(|s| s.clear())
                        >
                            {if is_ru { "Снять выбор" } else { "Clear" }}
                        </button>
                    </div>
                </Show>

                <div class="mt-5">
                    <DataGrid
                        columns=columns
                        data=Signal::derive(move || filtered_products.get())
                        key_fn=|item: &PricingProductListItem| item.id.clone()
                        cell_renderer=cell_renderer
                        is_loading=Signal::derive(move || busy.get() || products.get().is_none())
                        empty_message=t(ui_locale_for_list.as_deref(), "pricing.list.empty", "No products match the current filters.")
                        selection=selection
                        pagination=pagination
                        filters=filters
                        on_filter_change=on_filters_change
                        on_row_click=on_row_click
                    />
                </div>
            </section>

                <PricingDetailSection
                    ui_locale=ui_locale
                    effective_locale=effective_locale
                    bootstrap=bootstrap
                    context_active_price_lists=context_active_price_lists
                    selected=Signal::derive(move || selected.get())
                    applied_resolution_context=Signal::derive(move || applied_resolution_context.get())
                    resolution_currency=resolution_currency
                    set_resolution_currency=set_resolution_currency
                    resolution_region_id=resolution_region_id
                    set_resolution_region_id=set_resolution_region_id
                    resolution_price_list_id=resolution_price_list_id
                    set_resolution_price_list_id=set_resolution_price_list_id
                    resolution_channel_id=resolution_channel_id
                    set_resolution_channel_id=set_resolution_channel_id
                    resolution_channel_slug=resolution_channel_slug
                    set_resolution_channel_slug=set_resolution_channel_slug
                    resolution_quantity=resolution_quantity
                    set_resolution_quantity=set_resolution_quantity
                    busy=busy
                    set_busy=set_busy
                    error=error
                    set_error=set_error
                    refresh_detail=refresh_detail
                    context_query_writer=context_query_writer
                    product_module_route_base=product_module_route_base
                    open_product=open_product
                />
            </div>
        </section>
    }
}
