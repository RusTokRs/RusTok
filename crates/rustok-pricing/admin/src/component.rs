use std::collections::BTreeSet;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_api::{AdminQueryKey, UiRouteContext};
use rustok_core::locale_tags_match;
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use uuid::Uuid;

use crate::i18n::t;
use crate::model::{
    PricingAdjustmentPreview, PricingAdminBootstrap, PricingChannelOption, PricingDiscountDraft,
    PricingEffectivePrice, PricingPrice, PricingPriceDraft, PricingPriceListOption,
    PricingProductDetail, PricingProductListItem, PricingResolutionContext, PricingVariant,
};

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

    let bootstrap = Resource::new(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            crate::api::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let products = Resource::new(
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

    let context_active_price_lists = Resource::new(
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
            crate::api::fetch_active_price_lists(
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
            match crate::api::fetch_product(
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
    let ui_locale_for_list_status = ui_locale.clone();
    let ui_locale_for_detail = ui_locale.clone();
    let ui_locale_for_variants = ui_locale.clone();
    let ui_locale_for_empty = ui_locale.clone();
    let ui_locale_for_context = ui_locale.clone();
    let ui_locale_for_price_list_select = ui_locale.clone();
    let ui_locale_for_channel_select = ui_locale.clone();
    let effective_locale_for_detail = effective_locale.clone();
    let product_module_route_base = route_context.module_route_base("product");
    let refresh_open_product = open_product.clone();
    let initial_open_product = open_product.clone();
    let list_query_writer = query_writer.clone();
    let context_query_writer = query_writer.clone();
    let context_query_writer_for_region = query_writer.clone();
    let context_query_writer_for_price_list = query_writer.clone();
    let context_query_writer_for_channel = query_writer.clone();
    let context_query_writer_for_quantity = query_writer.clone();
    let refresh_detail = Callback::new(move |_| {
        set_refresh_nonce.update(|value| *value += 1);
        if let Some(product_id) = selected_id.get_untracked() {
            refresh_open_product.run(product_id);
        }
    });
    Effect::new(move |_| {
        match selected_product_query.get() {
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
                                        let item_query_writer = list_query_writer.clone();
                                        let item_locale = ui_locale_for_list_status.clone();
                                        let item_locale_for_meta = item_locale.clone();
                                        let item_locale_for_profile = item_locale.clone();
                                        let shipping_profile = product.shipping_profile_slug.clone();
                                        let profile_label = shipping_profile
                                            .unwrap_or_else(|| t(item_locale_for_profile.as_deref(), "pricing.common.unassigned", "unassigned"));
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

                    <div class="rounded-2xl border border-border bg-background p-5">
                        <div class="space-y-2">
                            <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">
                                {t(ui_locale.as_deref(), "pricing.detail.contextTitle", "Effective price context")}
                            </h4>
                            <p class="text-sm text-muted-foreground">
                                {t(ui_locale.as_deref(), "pricing.detail.contextSubtitle", "Apply currency, region and quantity to resolve the effective admin-side price for each variant without leaving the pricing module.")}
                            </p>
                        </div>
                        <div class="mt-4 grid gap-3 md:grid-cols-[140px_minmax(0,1fr)_minmax(0,1fr)_minmax(0,1fr)_120px_auto]">
                            <input
                                class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                placeholder=t(ui_locale.as_deref(), "pricing.detail.currencyInput", "Currency")
                                prop:value=move || resolution_currency.get()
                                on:input=move |ev| {
                                    let next_value = event_target_value(&ev);
                                    set_resolution_currency.set(next_value.clone());
                                    if next_value.trim().is_empty() {
                                        context_query_writer.clear_key(AdminQueryKey::Currency.as_str());
                                    } else {
                                        context_query_writer.replace_value(AdminQueryKey::Currency.as_str(), next_value);
                                    }
                                }
                            />
                            <input
                                class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                placeholder=t(ui_locale.as_deref(), "pricing.detail.regionInput", "Region UUID")
                                prop:value=move || resolution_region_id.get()
                                on:input=move |ev| {
                                    let next_value = event_target_value(&ev);
                                    set_resolution_region_id.set(next_value.clone());
                                    if next_value.trim().is_empty() {
                                        context_query_writer_for_region.clear_key(AdminQueryKey::RegionId.as_str());
                                    } else {
                                        context_query_writer_for_region.replace_value(AdminQueryKey::RegionId.as_str(), next_value);
                                    }
                                }
                            />
                            <select
                                class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                prop:value=move || resolution_price_list_id.get()
                                on:change=move |ev| {
                                    let next_value = event_target_value(&ev);
                                    set_resolution_price_list_id.set(next_value.clone());
                                    if next_value.trim().is_empty() {
                                        context_query_writer_for_price_list.clear_key(AdminQueryKey::PriceListId.as_str());
                                    } else {
                                        context_query_writer_for_price_list.replace_value(AdminQueryKey::PriceListId.as_str(), next_value);
                                    }
                                }
                            >
                                <option value="">{t(ui_locale.as_deref(), "pricing.detail.basePriceListFallback", "base prices")}</option>
                                {move || {
                                    let selected_price_list_id = resolution_price_list_id.get();
                                    let mut options = context_active_price_lists
                                        .get()
                                        .and_then(Result::ok)
                                        .unwrap_or_default();
                                    options.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));

                                    let mut views = Vec::new();
                                    if !selected_price_list_id.trim().is_empty()
                                        && !options.iter().any(|option| option.id == selected_price_list_id)
                                    {
                                        let fallback_label = format!(
                                            "{} ({})",
                                            t(ui_locale_for_price_list_select.as_deref(), "pricing.detail.customPriceList", "custom price list"),
                                            selected_price_list_id
                                        );
                                        views.push(view! {
                                            <option value=selected_price_list_id.clone()>{fallback_label}</option>
                                        }.into_any());
                                    }

                                    views.extend(options.into_iter().map(|option| {
                                        let label = format_price_list_option_label(ui_locale_for_price_list_select.as_deref(), &option);
                                        view! {
                                            <option value=option.id>{label}</option>
                                        }.into_any()
                                    }));

                                    views.into_iter().collect_view()
                                }}
                            </select>
                            <select
                                class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                prop:value=move || {
                                    let available_channels = bootstrap
                                        .get()
                                        .and_then(Result::ok)
                                        .map(|value| value.available_channels)
                                        .unwrap_or_default();
                                    selected_channel_key(
                                        resolution_channel_id.get().as_str(),
                                        resolution_channel_slug.get().as_str(),
                                        available_channels.as_slice(),
                                    )
                                }
                                on:change=move |ev| {
                                    let selected_key = event_target_value(&ev);
                                    let available_channels = bootstrap
                                        .get_untracked()
                                        .and_then(Result::ok)
                                        .map(|value| value.available_channels)
                                        .unwrap_or_default();
                                    let current_channel_id = resolution_channel_id.get_untracked();
                                    let current_channel_slug = resolution_channel_slug.get_untracked();
                                    let (next_channel_id, next_channel_slug) = apply_selected_channel_option(
                                        selected_key.as_str(),
                                        current_channel_id.as_str(),
                                        current_channel_slug.as_str(),
                                        available_channels.as_slice(),
                                    );
                                    set_resolution_channel_id.set(next_channel_id.clone());
                                    set_resolution_channel_slug.set(next_channel_slug.clone());
                                    context_query_writer_for_channel.update(
                                        vec![
                                            (
                                                AdminQueryKey::ChannelId.as_str().to_string(),
                                                (!next_channel_id.trim().is_empty())
                                                    .then_some(next_channel_id),
                                            ),
                                            (
                                                AdminQueryKey::ChannelSlug.as_str().to_string(),
                                                (!next_channel_slug.trim().is_empty())
                                                    .then_some(next_channel_slug),
                                            ),
                                        ],
                                        true,
                                    );
                                }
                            >
                                <option value=GLOBAL_CHANNEL_KEY>
                                    {t(ui_locale.as_deref(), "pricing.channel.global", "Global channel")}
                                </option>
                                {move || {
                                    let available_channels = bootstrap
                                        .get()
                                        .and_then(Result::ok)
                                        .map(|value| value.available_channels)
                                        .unwrap_or_default();
                                    let selected_key = selected_channel_key(
                                        resolution_channel_id.get().as_str(),
                                        resolution_channel_slug.get().as_str(),
                                        available_channels.as_slice(),
                                    );

                                    let mut views = Vec::new();
                                    if selected_key == LEGACY_CHANNEL_KEY {
                                        let label = format!(
                                            "{}: {}",
                                            t(
                                                ui_locale_for_channel_select.as_deref(),
                                                "pricing.channel.legacy",
                                                "Legacy scope",
                                            ),
                                            format_channel_scope_text(
                                                None,
                                                normalize_channel_value(
                                                    resolution_channel_id.get().as_str(),
                                                )
                                                .as_deref(),
                                                normalize_channel_value(
                                                    resolution_channel_slug.get().as_str(),
                                                )
                                                .as_deref(),
                                            )
                                            .unwrap_or_else(|| {
                                                t(
                                                    ui_locale_for_channel_select.as_deref(),
                                                    "pricing.common.notSet",
                                                    "not set",
                                                )
                                            }),
                                        );
                                        views.push(
                                            view! { <option value=LEGACY_CHANNEL_KEY>{label}</option> }
                                                .into_any(),
                                        );
                                    }

                                    views.extend(available_channels.into_iter().map(|option| {
                                            let label = format_channel_option_label(
                                                ui_locale_for_channel_select.as_deref(),
                                                &option,
                                            );
                                            view! { <option value=option.id>{label}</option> }.into_any()
                                        }));

                                    views.into_iter().collect_view()
                                }}
                            </select>
                            <input
                                class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                placeholder=t(ui_locale.as_deref(), "pricing.detail.quantityInput", "Qty")
                                prop:value=move || resolution_quantity.get()
                                on:input=move |ev| {
                                    let next_value = event_target_value(&ev);
                                    set_resolution_quantity.set(next_value.clone());
                                    if next_value.trim().is_empty() {
                                        context_query_writer_for_quantity.clear_key(AdminQueryKey::Quantity.as_str());
                                    } else {
                                        context_query_writer_for_quantity.replace_value(AdminQueryKey::Quantity.as_str(), next_value);
                                    }
                                }
                            />
                            <button
                                type="button"
                                class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                disabled=move || busy.get() || selected_id.get().is_none()
                                on:click=move |_| {
                                    if let Some(product_id) = selected_id.get() {
                                        open_product.run(product_id);
                                    }
                                }
                            >
                                {t(ui_locale.as_deref(), "pricing.action.applyContext", "Apply context")}
                            </button>
                        </div>
                        {move || {
                            applied_resolution_context.get().map(|context| {
                                let active_price_lists = context_active_price_lists
                                    .get()
                                    .and_then(Result::ok)
                                    .unwrap_or_default();
                                view! {
                                    <div class="mt-4 inline-flex flex-wrap items-center gap-2 rounded-2xl border border-primary/20 bg-primary/5 px-4 py-2 text-xs text-primary">
                                        <span class="font-semibold uppercase tracking-[0.16em]">
                                            {t(ui_locale_for_context.as_deref(), "pricing.detail.effectiveContext", "effective context")}
                                        </span>
                                        <span>{format_effective_context(ui_locale_for_context.as_deref(), &context, active_price_lists.as_slice())}</span>
                                    </div>
                                }
                            })
                        }}
                        {move || {
                            let selected_price_list_id = applied_resolution_context
                                .get()
                                .and_then(|context| context.price_list_id);
                            let active_price_lists = context_active_price_lists
                                .get()
                                .and_then(Result::ok)
                                .unwrap_or_default();

                            selected_price_list_id
                                .and_then(|price_list_id| {
                                    active_price_lists
                                        .into_iter()
                                        .find(|option| option.id == price_list_id)
                                })
                                .map(|price_list| {
                                    let available_channels = bootstrap
                                        .get()
                                        .and_then(Result::ok)
                                        .map(|value| value.available_channels)
                                        .unwrap_or_default();
                                    view! {
                                        <div class="mt-4">
                                            <PriceListRuleEditor
                                                locale=ui_locale.clone()
                                                price_list=price_list
                                                available_channels=available_channels
                                                on_saved=refresh_detail.clone()
                                            />
                                        </div>
                                    }
                                })
                        }}
                    </div>

                    <Show when=move || error.get().is_some()>
                        <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                            {move || error.get().unwrap_or_default()}
                        </div>
                    </Show>

                    {move || selected.get().map(|detail| {
                        let resolved_translation = pricing_translation_for_locale(
                            detail.translations.as_slice(),
                            effective_locale_for_detail.as_deref(),
                        );
                        let product_title = resolved_translation
                            .map(|item| item.title.clone())
                            .unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "pricing.detail.untitled", "Untitled"));
                        let product_handle = resolved_translation
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
                        let seller_id = detail
                            .seller_id
                            .clone()
                            .unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "pricing.common.notSet", "not set"));
                        let product_type = detail
                            .product_type
                            .clone()
                            .unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "pricing.common.notSet", "not set"));
                        let status_label = localized_product_status(ui_locale_for_detail.as_deref(), detail.status.as_str());
                        let available_channels = bootstrap
                            .get()
                            .and_then(Result::ok)
                            .map(|value| value.available_channels)
                            .unwrap_or_default();
                        let price_list_options = context_active_price_lists
                            .get()
                            .and_then(Result::ok)
                            .unwrap_or_default();
                        let product_href = build_product_admin_href(
                            product_module_route_base.as_str(),
                            detail.id.as_str(),
                        );
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
                                            <p class="text-sm text-muted-foreground">{format!("{}: {}", t(ui_locale_for_detail.as_deref(), "pricing.detail.seller", "seller"), seller_id)}</p>
                                            <p class="text-xs text-muted-foreground">{format!("shipping profile: {shipping_profile} | updated {}", detail.updated_at)}</p>
                                        </div>
                                        <div class="text-right text-xs text-muted-foreground">
                                            <p>{format!("created {}", detail.created_at)}</p>
                                            <p>{format!("published {}", detail.published_at.unwrap_or_else(|| "-".to_string()))}</p>
                                            <a class="mt-2 inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent" href=product_href>
                                                {t(ui_locale_for_detail.as_deref(), "pricing.detail.openProduct", "Open product module")}
                                            </a>
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
                                    {t(ui_locale_for_detail.as_deref(), "pricing.detail.transportGap", "This route already owns pricing visibility, effective-context inspection, base-row and active price-list write actions, plus price-list rule and scope editing. The remaining backlog is the broader promotions engine, not the core pricing transport path.")}
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
                                            let variant_available_channels = available_channels.clone();
                                            let variant_price_list_options = price_list_options.clone();
                                            let health_label = pricing_health_label(variant_locale.as_deref(), &variant);
                                            let price_table = format_variant_prices(
                                                variant_locale.as_deref(),
                                                variant.prices.as_slice(),
                                                variant_price_list_options.as_slice(),
                                            );
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
                                                            {variant.effective_price.as_ref().map(|price| view! {
                                                                <p class="text-sm font-medium text-foreground">
                                                                    {format_effective_price(variant_locale.as_deref(), price)}
                                                                </p>
                                                            })}
                                                            <div class="pt-2">
                                                                <VariantPriceEditors
                                                                    locale=variant_locale.clone()
                                                                    product_id=detail.id.clone()
                                                                    variant=variant.clone()
                                                                    available_channels=variant_available_channels.clone()
                                                                    default_currency=applied_resolution_context
                                                                        .get()
                                                                        .map(|context| context.currency_code)
                                                                    selected_price_list_id=applied_resolution_context
                                                                        .get()
                                                                        .and_then(|context| context.price_list_id)
                                                                    on_saved=open_product.clone()
                                                                />
                                                            </div>
                                                            <div class="pt-2">
                                                                <VariantDiscountEditor
                                                                    locale=variant_locale.clone()
                                                                    product_id=detail.id.clone()
                                                                    variant=variant.clone()
                                                                    available_channels=variant_available_channels.clone()
                                                                    default_currency=applied_resolution_context
                                                                        .get()
                                                                        .map(|context| context.currency_code)
                                                                    selected_price_list_id=applied_resolution_context
                                                                        .get()
                                                                        .and_then(|context| context.price_list_id)
                                                                    on_saved=open_product.clone()
                                                                />
                                                            </div>
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
        .filter(|variant| {
            variant
                .effective_price
                .as_ref()
                .map(|price| price.on_sale)
                .unwrap_or(false)
                || variant.prices.iter().any(|price| price.on_sale)
        })
        .count();
    let currency_count = variants
        .iter()
        .flat_map(|variant| {
            variant
                .prices
                .iter()
                .map(|price| price.currency_code.clone())
                .chain(
                    variant
                        .effective_price
                        .iter()
                        .map(|price| price.currency_code.clone()),
                )
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

fn pricing_translation_for_locale<'a>(
    translations: &'a [crate::model::PricingProductTranslation],
    requested_locale: Option<&str>,
) -> Option<&'a crate::model::PricingProductTranslation> {
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
    let seller_id = product
        .seller_id
        .clone()
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set"));
    format!(
        "handle: {} | vendor: {} | type: {} | seller: {}",
        product.handle, vendor, product_type, seller_id
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

fn format_variant_prices(
    locale: Option<&str>,
    prices: &[PricingPrice],
    price_list_options: &[PricingPriceListOption],
) -> String {
    if prices.is_empty() {
        return t(locale, "pricing.common.noPricing", "no pricing");
    }

    prices
        .iter()
        .map(|price| {
            let amount = match price.compare_at_amount.as_deref() {
                Some(compare_at) if !compare_at.is_empty() => {
                    format!(
                        "{} {} (compare-at {})",
                        price.currency_code, price.amount, compare_at
                    )
                }
                _ => format!("{} {}", price.currency_code, price.amount),
            };
            let discount_suffix = format_discount_suffix(price.discount_percent.as_deref());
            format!(
                "{} [{}]",
                format!("{amount}{discount_suffix}"),
                format_price_row_scope(locale, price, price_list_options)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn pricing_health_label(locale: Option<&str>, variant: &PricingVariant) -> String {
    if variant.effective_price.is_some() {
        t(locale, "pricing.health.effective", "Effective")
    } else if variant.prices.is_empty() {
        t(locale, "pricing.health.missing", "No pricing")
    } else if variant.prices.iter().any(|price| price.on_sale) {
        t(locale, "pricing.health.sale", "On sale")
    } else {
        t(locale, "pricing.health.base", "Base price")
    }
}

fn pricing_health_badge(variant: &PricingVariant) -> &'static str {
    if variant.effective_price.is_some() {
        "border-primary/30 bg-primary/5 text-primary"
    } else if variant.prices.is_empty() {
        "border-rose-200 bg-rose-50 text-rose-700"
    } else if variant.prices.iter().any(|price| price.on_sale) {
        "border-amber-200 bg-amber-50 text-amber-700"
    } else {
        "border-emerald-200 bg-emerald-50 text-emerald-700"
    }
}

fn format_price_list_option_label(locale: Option<&str>, option: &PricingPriceListOption) -> String {
    let mut label = format!(
        "{} ({})",
        option.name,
        format!(
            "{} {}",
            t(locale, "pricing.detail.priceListTypeLabel", "type"),
            option.list_type
        )
    );
    if option.rule_kind.as_deref() == Some("percentage_discount") {
        if let Some(adjustment_percent) = option.adjustment_percent.as_deref() {
            label.push_str(format!(" | -{adjustment_percent}%").as_str());
        }
    }
    if let Some(channel_scope) = format_channel_scope_text(
        locale,
        option.channel_id.as_deref(),
        option.channel_slug.as_deref(),
    ) {
        label.push_str(format!(" | {channel_scope}").as_str());
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
            "pricing.detail.globalRegionFallback",
            "global region",
        )
    });
    let price_list = resolve_price_list_label(
        locale,
        context.price_list_id.as_deref(),
        price_list_options,
        "pricing.detail.basePriceListFallback",
        "base prices",
    );
    let mut parts = vec![
        format!(
            "{} {}",
            t(locale, "pricing.detail.currencyInput", "currency"),
            context.currency_code
        ),
        format!(
            "{} {}",
            t(locale, "pricing.detail.regionInput", "region"),
            region
        ),
        format!(
            "{} {}",
            t(locale, "pricing.detail.priceListInput", "price list"),
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
        t(locale, "pricing.detail.quantityInput", "qty"),
        context.quantity
    ));
    parts.join(" | ")
}

fn format_effective_price(locale: Option<&str>, price: &PricingEffectivePrice) -> String {
    let base = if let Some(compare_at_amount) = price.compare_at_amount.as_deref() {
        format!(
            "{} {} (compare-at {})",
            price.currency_code, price.amount, compare_at_amount
        )
    } else {
        format!("{} {}", price.currency_code, price.amount)
    };

    let scope = format_price_scope(locale, price.min_quantity, price.max_quantity);
    let discount_suffix = format_discount_suffix(price.discount_percent.as_deref());

    format!(
        "{}{} | {} {}",
        base,
        discount_suffix,
        t(locale, "pricing.detail.effectiveContext", "effective"),
        scope
    )
}

fn format_discount_suffix(discount_percent: Option<&str>) -> String {
    discount_percent
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" (-{value}%)"))
        .unwrap_or_default()
}

fn format_price_scope(
    locale: Option<&str>,
    min_quantity: Option<i32>,
    max_quantity: Option<i32>,
) -> String {
    match (min_quantity, max_quantity) {
        (Some(min_quantity), Some(max_quantity)) => format!(
            "{} {}-{}",
            t(locale, "pricing.detail.quantityRange", "tier"),
            min_quantity,
            max_quantity
        ),
        (Some(min_quantity), None) => format!(
            "{} {}+",
            t(locale, "pricing.detail.quantityRange", "tier"),
            min_quantity
        ),
        (None, Some(max_quantity)) => format!(
            "{} 1-{}",
            t(locale, "pricing.detail.quantityRange", "tier"),
            max_quantity
        ),
        _ => t(locale, "pricing.detail.quantityDefault", "default quantity").to_string(),
    }
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

    let channel_label = t(locale, "pricing.detail.channelInput", "channel");
    match (channel_slug, channel_id) {
        (Some(channel_slug), Some(channel_id)) => {
            Some(format!("{channel_label} {channel_slug} ({channel_id})"))
        }
        (Some(channel_slug), None) => Some(format!("{channel_label} {channel_slug}")),
        (None, Some(channel_id)) => Some(format!("{channel_label} {channel_id}")),
        (None, None) => None,
    }
}

const GLOBAL_CHANNEL_KEY: &str = "__global__";
const LEGACY_CHANNEL_KEY: &str = "__legacy__";

fn normalize_channel_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn selected_channel_key(
    channel_id: &str,
    channel_slug: &str,
    available_channels: &[PricingChannelOption],
) -> String {
    let normalized_channel_id = normalize_channel_value(channel_id);
    let normalized_channel_slug = normalize_channel_value(channel_slug);

    if normalized_channel_id.is_none() && normalized_channel_slug.is_none() {
        return GLOBAL_CHANNEL_KEY.to_string();
    }

    if let Some(option) = available_channels.iter().find(|option| {
        normalized_channel_id.as_deref() == Some(option.id.as_str())
            || normalized_channel_slug.as_deref() == Some(option.slug.as_str())
    }) {
        return option.id.clone();
    }

    LEGACY_CHANNEL_KEY.to_string()
}

fn apply_selected_channel_option(
    selected_key: &str,
    fallback_channel_id: &str,
    fallback_channel_slug: &str,
    available_channels: &[PricingChannelOption],
) -> (String, String) {
    match selected_key {
        GLOBAL_CHANNEL_KEY => (String::new(), String::new()),
        LEGACY_CHANNEL_KEY => (
            fallback_channel_id.to_string(),
            fallback_channel_slug.to_string(),
        ),
        _ => available_channels
            .iter()
            .find(|option| option.id == selected_key)
            .map(|option| (option.id.clone(), option.slug.clone()))
            .unwrap_or_default(),
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

fn format_price_row_scope(
    locale: Option<&str>,
    price: &PricingPrice,
    price_list_options: &[PricingPriceListOption],
) -> String {
    let tier_scope = format_price_scope(locale, price.min_quantity, price.max_quantity);
    let channel_scope = format_channel_scope_text(
        locale,
        price.channel_id.as_deref(),
        price.channel_slug.as_deref(),
    );
    if price.price_list_id.is_some() {
        let price_list_label = resolve_price_list_label(
            locale,
            price.price_list_id.as_deref(),
            price_list_options,
            "pricing.detail.priceListInput",
            "price list",
        );
        match channel_scope {
            Some(channel_scope) => format!("{price_list_label} | {channel_scope} | {tier_scope}"),
            None => format!("{price_list_label} | {tier_scope}"),
        }
    } else {
        match channel_scope {
            Some(channel_scope) => format!("base | {channel_scope} | {tier_scope}"),
            None => format!("base | {tier_scope}"),
        }
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

fn normalized_currency_code(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.len() == 3 {
        Some(trimmed.to_ascii_uppercase())
    } else {
        None
    }
}

fn normalized_region_id(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Uuid::parse_str(trimmed).ok().map(|_| trimmed.to_string())
    }
}

fn normalized_price_list_id(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Uuid::parse_str(trimmed).ok().map(|_| trimmed.to_string())
    }
}

fn normalized_quantity(value: String) -> Option<i32> {
    value
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|quantity| *quantity > 0)
}

fn build_product_admin_href(module_route_base: &str, product_id: &str) -> String {
    format!("{module_route_base}?id={product_id}")
}

fn build_resolution_context(
    currency_value: String,
    region_value: String,
    price_list_value: String,
    channel_id_value: String,
    channel_slug_value: String,
    quantity_value: String,
) -> Option<PricingResolutionContext> {
    let currency_code = normalized_currency_code(currency_value)?;
    Some(PricingResolutionContext {
        currency_code,
        region_id: normalized_region_id(region_value),
        price_list_id: normalized_price_list_id(price_list_value),
        channel_id: normalize_channel_value(&channel_id_value),
        channel_slug: normalize_channel_value(&channel_slug_value),
        quantity: normalized_quantity(quantity_value).unwrap_or(1),
    })
}

fn status_badge(status: &str) -> &'static str {
    match status {
        "ACTIVE" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "ARCHIVED" => "border-slate-200 bg-slate-100 text-slate-700",
        _ => "border-amber-200 bg-amber-50 text-amber-700",
    }
}

#[component]
fn VariantPriceEditors(
    locale: Option<String>,
    product_id: String,
    variant: PricingVariant,
    available_channels: Vec<PricingChannelOption>,
    default_currency: Option<String>,
    selected_price_list_id: Option<String>,
    on_saved: Callback<String>,
) -> impl IntoView {
    let add_price_title = t(
        locale.as_deref(),
        "pricing.edit.addPrice",
        "Add or overwrite currency",
    );
    let existing_editors = variant
        .prices
        .iter()
        .cloned()
        .map(|price| {
            let compare_at_amount = price.compare_at_amount.clone().unwrap_or_default();
            let title = format!(
                "{} ({})",
                t(locale.as_deref(), "pricing.edit.updatePrice", "Update price"),
                format_price_scope(locale.as_deref(), price.min_quantity, price.max_quantity)
            );
            view! {
                <VariantPriceEditor
                    locale=locale.clone()
                    product_id=product_id.clone()
                    variant_id=variant.id.clone()
                    available_channels=available_channels.clone()
                    draft=PricingPriceDraft {
                        currency_code: price.currency_code,
                        amount: price.amount,
                        compare_at_amount,
                        price_list_id: price.price_list_id.clone().unwrap_or_default(),
                        channel_id: price.channel_id.clone().unwrap_or_default(),
                        channel_slug: price.channel_slug.clone().unwrap_or_default(),
                        min_quantity: price.min_quantity.map(|value| value.to_string()).unwrap_or_default(),
                        max_quantity: price.max_quantity.map(|value| value.to_string()).unwrap_or_default(),
                    }
                    title=title
                    on_saved=on_saved.clone()
                />
            }
        })
        .collect_view();
    let add_currency = default_currency
        .or_else(|| {
            variant
                .prices
                .first()
                .map(|price| price.currency_code.clone())
        })
        .unwrap_or_default();

    view! {
        <div class="space-y-2 rounded-xl border border-border/70 bg-background/70 p-3">
            <div class="flex items-center justify-between gap-3">
                <h6 class="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                    {t(locale.as_deref(), "pricing.edit.title", "Base pricing")}
                </h6>
                <span class="text-xs text-muted-foreground">
                    {t(locale.as_deref(), "pricing.edit.scope", "Base-price write path only")}
                </span>
            </div>
            {existing_editors}
            <VariantPriceEditor
                locale=locale.clone()
                product_id=product_id
                variant_id=variant.id
                available_channels=available_channels
                draft=PricingPriceDraft {
                    currency_code: add_currency,
                    amount: String::new(),
                    compare_at_amount: String::new(),
                    price_list_id: selected_price_list_id.unwrap_or_default(),
                    channel_id: String::new(),
                    channel_slug: String::new(),
                    min_quantity: String::new(),
                    max_quantity: String::new(),
                }
                title=add_price_title
                on_saved=on_saved
            />
        </div>
    }
}

#[component]
fn VariantDiscountEditor(
    locale: Option<String>,
    product_id: String,
    variant: PricingVariant,
    available_channels: Vec<PricingChannelOption>,
    default_currency: Option<String>,
    selected_price_list_id: Option<String>,
    on_saved: Callback<String>,
) -> impl IntoView {
    let base_currency = variant
        .prices
        .iter()
        .find(|price| {
            price.price_list_id.is_none()
                && price.min_quantity.is_none()
                && price.max_quantity.is_none()
        })
        .map(|price| price.currency_code.clone())
        .or(default_currency)
        .unwrap_or_default();
    let selected_scope_price = variant.prices.iter().find(|price| {
        price.currency_code == base_currency
            && price.min_quantity.is_none()
            && price.max_quantity.is_none()
            && match selected_price_list_id.as_deref().map(str::trim) {
                Some(price_list_id) if !price_list_id.is_empty() => {
                    price.price_list_id.as_deref() == Some(price_list_id)
                }
                _ => price.price_list_id.is_none(),
            }
    });
    let initial_channel_id = selected_scope_price
        .and_then(|price| price.channel_id.clone())
        .unwrap_or_default();
    let initial_channel_slug = selected_scope_price
        .and_then(|price| price.channel_slug.clone())
        .unwrap_or_default();

    let (currency_code, set_currency_code) = signal(base_currency);
    let (discount_percent, set_discount_percent) = signal(String::new());
    let (channel_id, set_channel_id) = signal(initial_channel_id);
    let (channel_slug, set_channel_slug) = signal(initial_channel_slug);
    let (preview, set_preview) = signal(Option::<PricingAdjustmentPreview>::None);
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let available_channels_for_value = available_channels.clone();
    let available_channels_for_change = available_channels.clone();
    let available_channels_for_options = available_channels.clone();
    let available_channels_for_legacy = available_channels.clone();
    let locale_for_preview = locale.clone();
    let locale_for_apply = locale.clone();
    let locale_for_button = locale.clone();
    let locale_for_apply_button = locale.clone();
    let locale_for_legacy_label = locale.clone();
    let legacy_option_label = Memo::new(move |_| {
        format!(
            "{}: {}",
            t(
                locale_for_legacy_label.as_deref(),
                "pricing.channel.legacy",
                "Legacy scope",
            ),
            format_channel_scope_text(
                None,
                normalize_channel_value(channel_id.get().as_str()).as_deref(),
                normalize_channel_value(channel_slug.get().as_str()).as_deref(),
            )
            .unwrap_or_else(|| {
                t(
                    locale_for_legacy_label.as_deref(),
                    "pricing.common.notSet",
                    "not set",
                )
            })
        )
    });
    let preview_selected_price_list_id = selected_price_list_id.clone();
    let apply_selected_price_list_id = selected_price_list_id;
    let preview_label = t(
        locale.as_deref(),
        "pricing.adjustment.previewLabel",
        "discount",
    );
    let (preview_label, _) = signal(preview_label);
    let preview_variant_id = variant.id.clone();
    let apply_variant_id = variant.id;

    view! {
        <div class="space-y-2 rounded-xl border border-border/70 bg-background/70 p-3">
            <div class="flex items-center justify-between gap-3">
                <h6 class="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                    {t(locale.as_deref(), "pricing.adjustment.title", "Canonical-row adjustment")}
                </h6>
                <span class="text-xs text-muted-foreground">
                    {t(locale.as_deref(), "pricing.adjustment.scope", "Applies only to the canonical row for the selected scope")}
                </span>
            </div>
            <div class="grid gap-2 md:grid-cols-[110px_140px_minmax(0,1fr)_auto_auto]">
                <input
                    class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                    placeholder=t(locale.as_deref(), "pricing.edit.currencyInput", "Currency")
                    prop:value=move || currency_code.get()
                    on:input=move |ev| set_currency_code.set(event_target_value(&ev))
                />
                <input
                    class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                    placeholder=t(locale.as_deref(), "pricing.adjustment.percentInput", "Discount %")
                    prop:value=move || discount_percent.get()
                    on:input=move |ev| set_discount_percent.set(event_target_value(&ev))
                />
                <select
                    class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                    prop:value=move || {
                        selected_channel_key(
                            channel_id.get().as_str(),
                            channel_slug.get().as_str(),
                            available_channels_for_value.as_slice(),
                        )
                    }
                    on:change=move |ev| {
                        let selected_key = event_target_value(&ev);
                        let current_channel_id = channel_id.get_untracked();
                        let current_channel_slug = channel_slug.get_untracked();
                        let (next_channel_id, next_channel_slug) = apply_selected_channel_option(
                            selected_key.as_str(),
                            current_channel_id.as_str(),
                            current_channel_slug.as_str(),
                            available_channels_for_change.as_slice(),
                        );
                        set_channel_id.set(next_channel_id);
                        set_channel_slug.set(next_channel_slug);
                    }
                >
                    <option value=GLOBAL_CHANNEL_KEY>
                        {t(locale.as_deref(), "pricing.channel.global", "Global channel")}
                    </option>
                    {available_channels_for_options
                        .iter()
                        .map(|option| {
                            view! {
                                <option value=option.id.clone()>
                                    {format_channel_option_label(locale.as_deref(), option)}
                                </option>
                            }
                        })
                        .collect_view()}
                    <Show when=move || {
                        selected_channel_key(
                            channel_id.get().as_str(),
                            channel_slug.get().as_str(),
                            available_channels_for_legacy.as_slice(),
                        ) == LEGACY_CHANNEL_KEY
                    }>
                        <option value=LEGACY_CHANNEL_KEY>
                            {move || legacy_option_label.get()}
                        </option>
                    </Show>
                </select>
                <button
                    type="button"
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=move |_| {
                        let payload = PricingDiscountDraft {
                            currency_code: currency_code.get_untracked(),
                            discount_percent: discount_percent.get_untracked(),
                            price_list_id: preview_selected_price_list_id
                                .clone()
                                .unwrap_or_default(),
                            channel_id: channel_id.get_untracked(),
                            channel_slug: channel_slug.get_untracked(),
                        };
                        let variant_id = preview_variant_id.clone();
                        let preview_error_label = t(locale_for_preview.as_deref(), "pricing.adjustment.previewError", "Failed to preview discount");
                        set_busy.set(true);
                        set_error.set(None);
                        spawn_local(async move {
                            match crate::api::preview_variant_discount(variant_id, payload).await {
                                Ok(value) => set_preview.set(Some(value)),
                                Err(err) => set_error.set(Some(format!("{preview_error_label}: {err}"))),
                            }
                            set_busy.set(false);
                        });
                    }
                >
                    {move || if busy.get() {
                        t(locale_for_button.as_deref(), "pricing.adjustment.previewing", "Previewing...")
                    } else {
                        t(locale_for_button.as_deref(), "pricing.adjustment.preview", "Preview")
                    }}
                </button>
                <button
                    type="button"
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=move |_| {
                        let payload = PricingDiscountDraft {
                            currency_code: currency_code.get_untracked(),
                            discount_percent: discount_percent.get_untracked(),
                            price_list_id: apply_selected_price_list_id
                                .clone()
                                .unwrap_or_default(),
                            channel_id: channel_id.get_untracked(),
                            channel_slug: channel_slug.get_untracked(),
                        };
                        let variant_id = apply_variant_id.clone();
                        let product_id = product_id.clone();
                        let on_saved = on_saved.clone();
                        let apply_error_label = t(locale_for_apply.as_deref(), "pricing.adjustment.applyError", "Failed to apply discount");
                        set_busy.set(true);
                        set_error.set(None);
                        spawn_local(async move {
                            match crate::api::apply_variant_discount(variant_id, payload).await {
                                Ok(value) => {
                                    set_preview.set(Some(value));
                                    on_saved.run(product_id);
                                }
                                Err(err) => set_error.set(Some(format!("{apply_error_label}: {err}"))),
                            }
                            set_busy.set(false);
                        });
                    }
                >
                    {move || if busy.get() {
                        t(locale_for_apply_button.as_deref(), "pricing.adjustment.applying", "Applying...")
                    } else {
                        t(locale_for_apply_button.as_deref(), "pricing.adjustment.apply", "Apply")
                    }}
                </button>
            </div>
            <Show when=move || preview.get().is_some()>
                <p class="text-xs text-muted-foreground">
                    {move || {
                        preview
                            .get()
                            .map(|preview| {
                                let preview_label = preview_label.get();
                                format_adjustment_preview(preview_label.as_str(), &preview)
                            })
                            .unwrap_or_default()
                    }}
                </p>
            </Show>
            <Show when=move || error.get().is_some()>
                <p class="text-xs text-destructive">{move || error.get().unwrap_or_default()}</p>
            </Show>
        </div>
    }
}

#[component]
fn PriceListRuleEditor(
    locale: Option<String>,
    price_list: PricingPriceListOption,
    available_channels: Vec<PricingChannelOption>,
    on_saved: Callback<()>,
) -> impl IntoView {
    let locale_for_save = locale.clone();
    let locale_for_scope_save = locale.clone();
    let locale_for_clear = locale.clone();
    let locale_for_button = locale.clone();
    let (adjustment_percent, set_adjustment_percent) =
        signal(price_list.adjustment_percent.clone().unwrap_or_default());
    let (channel_id, set_channel_id) = signal(price_list.channel_id.clone().unwrap_or_default());
    let (channel_slug, set_channel_slug) =
        signal(price_list.channel_slug.clone().unwrap_or_default());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let available_channels_for_value = available_channels.clone();
    let available_channels_for_change = available_channels.clone();
    let available_channels_for_options = available_channels.clone();
    let available_channels_for_legacy = available_channels.clone();
    let locale_for_legacy_label = locale.clone();
    let legacy_option_label = Memo::new(move |_| {
        format!(
            "{}: {}",
            t(
                locale_for_legacy_label.as_deref(),
                "pricing.channel.legacy",
                "Legacy scope",
            ),
            format_channel_scope_text(
                None,
                normalize_channel_value(channel_id.get().as_str()).as_deref(),
                normalize_channel_value(channel_slug.get().as_str()).as_deref(),
            )
            .unwrap_or_else(|| {
                t(
                    locale_for_legacy_label.as_deref(),
                    "pricing.common.notSet",
                    "not set",
                )
            })
        )
    });
    let save_price_list_id = price_list.id.clone();
    let clear_price_list_id = price_list.id.clone();
    let save_scope_price_list_id = price_list.id.clone();

    view! {
        <div class="space-y-3 rounded-2xl border border-border bg-background p-4">
            <div class="space-y-1">
                <h5 class="text-sm font-semibold text-card-foreground">
                    {t(locale.as_deref(), "pricing.rule.title", "Active price-list rule")}
                </h5>
                <p class="text-xs text-muted-foreground">
                    {format_price_list_option_label(locale.as_deref(), &price_list)}
                </p>
                <p class="text-xs text-muted-foreground">
                    {t(locale.as_deref(), "pricing.rule.subtitle", "Configure a percentage rule that resolves from base prices when this active price list has no explicit override row.")}
                </p>
            </div>
            <div class="grid gap-2 md:grid-cols-[160px_auto_auto]">
                <input
                    class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                    placeholder=t(locale.as_deref(), "pricing.rule.percentInput", "Rule discount %")
                    prop:value=move || adjustment_percent.get()
                    on:input=move |ev| set_adjustment_percent.set(event_target_value(&ev))
                />
                <button
                    type="button"
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=move |_| {
                        let payload = crate::model::PricingPriceListRuleDraft {
                            adjustment_percent: adjustment_percent.get_untracked(),
                        };
                        let price_list_id = save_price_list_id.clone();
                        let on_saved = on_saved.clone();
                        let save_error_label = t(locale_for_save.as_deref(), "pricing.rule.saveError", "Failed to save price-list rule");
                        set_busy.set(true);
                        set_error.set(None);
                        spawn_local(async move {
                            match crate::api::update_price_list_rule(price_list_id, payload).await {
                                Ok(updated) => {
                                    set_adjustment_percent.set(updated.adjustment_percent.unwrap_or_default());
                                    on_saved.run(());
                                }
                                Err(err) => set_error.set(Some(format!("{save_error_label}: {err}"))),
                            }
                            set_busy.set(false);
                        });
                    }
                >
                    {move || if busy.get() {
                        t(locale_for_button.as_deref(), "pricing.rule.saving", "Saving...")
                    } else {
                        t(locale_for_button.as_deref(), "pricing.rule.save", "Save rule")
                    }}
                </button>
                <button
                    type="button"
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=move |_| {
                        let payload = crate::model::PricingPriceListRuleDraft {
                            adjustment_percent: String::new(),
                        };
                        let price_list_id = clear_price_list_id.clone();
                        let on_saved = on_saved.clone();
                        let save_error_label = t(locale_for_clear.as_deref(), "pricing.rule.saveError", "Failed to save price-list rule");
                        set_busy.set(true);
                        set_error.set(None);
                        spawn_local(async move {
                            match crate::api::update_price_list_rule(price_list_id, payload).await {
                                Ok(_) => {
                                    set_adjustment_percent.set(String::new());
                                    on_saved.run(());
                                }
                                Err(err) => set_error.set(Some(format!("{save_error_label}: {err}"))),
                            }
                            set_busy.set(false);
                        });
                    }
                >
                    {t(locale.as_deref(), "pricing.rule.clear", "Clear rule")}
                </button>
            </div>
            <div class="grid gap-2 md:grid-cols-[minmax(0,1fr)_auto]">
                <select
                    class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                    prop:value=move || {
                        selected_channel_key(
                            channel_id.get().as_str(),
                            channel_slug.get().as_str(),
                            available_channels_for_value.as_slice(),
                        )
                    }
                    on:change=move |ev| {
                        let selected_key = event_target_value(&ev);
                        let current_channel_id = channel_id.get_untracked();
                        let current_channel_slug = channel_slug.get_untracked();
                        let (next_channel_id, next_channel_slug) = apply_selected_channel_option(
                            selected_key.as_str(),
                            current_channel_id.as_str(),
                            current_channel_slug.as_str(),
                            available_channels_for_change.as_slice(),
                        );
                        set_channel_id.set(next_channel_id);
                        set_channel_slug.set(next_channel_slug);
                    }
                >
                    <option value=GLOBAL_CHANNEL_KEY>
                        {t(locale.as_deref(), "pricing.channel.global", "Global channel")}
                    </option>
                    {available_channels_for_options
                        .iter()
                        .map(|option| {
                            view! {
                                <option value=option.id.clone()>
                                    {format_channel_option_label(locale.as_deref(), option)}
                                </option>
                            }
                        })
                        .collect_view()}
                    <Show when=move || {
                        selected_channel_key(
                            channel_id.get().as_str(),
                            channel_slug.get().as_str(),
                            available_channels_for_legacy.as_slice(),
                        ) == LEGACY_CHANNEL_KEY
                    }>
                        <option value=LEGACY_CHANNEL_KEY>
                            {move || legacy_option_label.get()}
                        </option>
                    </Show>
                </select>
                <button
                    type="button"
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=move |_| {
                        let payload = crate::model::PricingPriceListScopeDraft {
                            channel_id: channel_id.get_untracked(),
                            channel_slug: channel_slug.get_untracked(),
                        };
                        let price_list_id = save_scope_price_list_id.clone();
                        let on_saved = on_saved.clone();
                        let save_error_label = t(locale_for_scope_save.as_deref(), "pricing.rule.scopeSaveError", "Failed to save price-list scope");
                        set_busy.set(true);
                        set_error.set(None);
                        spawn_local(async move {
                            match crate::api::update_price_list_scope(price_list_id, payload).await {
                                Ok(updated) => {
                                    set_channel_id.set(updated.channel_id.unwrap_or_default());
                                    set_channel_slug.set(updated.channel_slug.unwrap_or_default());
                                    on_saved.run(());
                                }
                                Err(err) => set_error.set(Some(format!("{save_error_label}: {err}"))),
                            }
                            set_busy.set(false);
                        });
                    }
                >
                    {t(locale.as_deref(), "pricing.rule.saveScope", "Save scope")}
                </button>
            </div>
            <Show when=move || error.get().is_some()>
                <p class="text-xs text-destructive">{move || error.get().unwrap_or_default()}</p>
            </Show>
        </div>
    }
}

#[component]
fn VariantPriceEditor(
    locale: Option<String>,
    product_id: String,
    variant_id: String,
    available_channels: Vec<PricingChannelOption>,
    draft: PricingPriceDraft,
    title: String,
    on_saved: Callback<String>,
) -> impl IntoView {
    let locale_for_save = locale.clone();
    let locale_for_button = locale.clone();
    let (currency_code, set_currency_code) = signal(draft.currency_code);
    let (amount, set_amount) = signal(draft.amount);
    let (compare_at_amount, set_compare_at_amount) = signal(draft.compare_at_amount);
    let price_list_id = draft.price_list_id;
    let (channel_id, set_channel_id) = signal(draft.channel_id);
    let (channel_slug, set_channel_slug) = signal(draft.channel_slug);
    let (min_quantity, set_min_quantity) = signal(draft.min_quantity);
    let (max_quantity, set_max_quantity) = signal(draft.max_quantity);
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let available_channels_for_value = available_channels.clone();
    let available_channels_for_change = available_channels.clone();
    let available_channels_for_options = available_channels.clone();
    let available_channels_for_legacy = available_channels.clone();
    let locale_for_legacy_label = locale.clone();
    let legacy_option_label = Memo::new(move |_| {
        format!(
            "{}: {}",
            t(
                locale_for_legacy_label.as_deref(),
                "pricing.channel.legacy",
                "Legacy scope",
            ),
            format_channel_scope_text(
                None,
                normalize_channel_value(channel_id.get().as_str()).as_deref(),
                normalize_channel_value(channel_slug.get().as_str()).as_deref(),
            )
            .unwrap_or_else(|| {
                t(
                    locale_for_legacy_label.as_deref(),
                    "pricing.common.notSet",
                    "not set",
                )
            })
        )
    });

    view! {
        <div class="space-y-2 rounded-lg border border-border/60 p-3">
            <p class="text-xs font-medium text-muted-foreground">{title}</p>
            <div class="grid gap-2 md:grid-cols-[110px_minmax(0,1fr)_minmax(0,1fr)_minmax(0,1fr)_120px_120px_auto]">
                <input
                    class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                    placeholder=t(locale.as_deref(), "pricing.edit.currencyInput", "Currency")
                    prop:value=move || currency_code.get()
                    on:input=move |ev| set_currency_code.set(event_target_value(&ev))
                />
                <input
                    class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                    placeholder=t(locale.as_deref(), "pricing.edit.amountInput", "Amount")
                    prop:value=move || amount.get()
                    on:input=move |ev| set_amount.set(event_target_value(&ev))
                />
                <input
                    class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                    placeholder=t(locale.as_deref(), "pricing.edit.compareAtInput", "Compare-at")
                    prop:value=move || compare_at_amount.get()
                    on:input=move |ev| set_compare_at_amount.set(event_target_value(&ev))
                />
                <select
                    class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                    prop:value=move || {
                        selected_channel_key(
                            channel_id.get().as_str(),
                            channel_slug.get().as_str(),
                            available_channels_for_value.as_slice(),
                        )
                    }
                    on:change=move |ev| {
                        let selected_key = event_target_value(&ev);
                        let current_channel_id = channel_id.get_untracked();
                        let current_channel_slug = channel_slug.get_untracked();
                        let (next_channel_id, next_channel_slug) = apply_selected_channel_option(
                            selected_key.as_str(),
                            current_channel_id.as_str(),
                            current_channel_slug.as_str(),
                            available_channels_for_change.as_slice(),
                        );
                        set_channel_id.set(next_channel_id);
                        set_channel_slug.set(next_channel_slug);
                    }
                >
                    <option value=GLOBAL_CHANNEL_KEY>
                        {t(locale.as_deref(), "pricing.channel.global", "Global channel")}
                    </option>
                    {available_channels_for_options
                        .iter()
                        .map(|option| {
                            view! {
                                <option value=option.id.clone()>
                                    {format_channel_option_label(locale.as_deref(), option)}
                                </option>
                            }
                        })
                        .collect_view()}
                    <Show when=move || {
                        selected_channel_key(
                            channel_id.get().as_str(),
                            channel_slug.get().as_str(),
                            available_channels_for_legacy.as_slice(),
                        ) == LEGACY_CHANNEL_KEY
                    }>
                        <option value=LEGACY_CHANNEL_KEY>
                            {move || legacy_option_label.get()}
                        </option>
                    </Show>
                </select>
                <input
                    class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                    placeholder=t(locale.as_deref(), "pricing.edit.minQuantityInput", "Min qty")
                    prop:value=move || min_quantity.get()
                    on:input=move |ev| set_min_quantity.set(event_target_value(&ev))
                />
                <input
                    class="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                    placeholder=t(locale.as_deref(), "pricing.edit.maxQuantityInput", "Max qty")
                    prop:value=move || max_quantity.get()
                    on:input=move |ev| set_max_quantity.set(event_target_value(&ev))
                />
                <button
                    type="button"
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=move |_| {
                        let variant_id = variant_id.clone();
                        let product_id = product_id.clone();
                        let payload = PricingPriceDraft {
                            currency_code: currency_code.get_untracked(),
                            amount: amount.get_untracked(),
                            compare_at_amount: compare_at_amount.get_untracked(),
                            price_list_id: price_list_id.clone(),
                            channel_id: channel_id.get_untracked(),
                            channel_slug: channel_slug.get_untracked(),
                            min_quantity: min_quantity.get_untracked(),
                            max_quantity: max_quantity.get_untracked(),
                        };
                        let on_saved = on_saved.clone();
                        let save_error_label = t(locale_for_save.as_deref(), "pricing.edit.saveError", "Failed to save price");
                        set_busy.set(true);
                        set_error.set(None);
                        spawn_local(async move {
                            match crate::api::update_variant_price(variant_id, payload).await {
                                Ok(()) => on_saved.run(product_id),
                                Err(err) => set_error.set(Some(format!("{save_error_label}: {err}"))),
                            }
                            set_busy.set(false);
                        });
                    }
                >
                    {move || if busy.get() {
                        t(locale_for_button.as_deref(), "pricing.edit.saving", "Saving...")
                    } else {
                        t(locale_for_button.as_deref(), "pricing.edit.save", "Save")
                    }}
                </button>
            </div>
            <Show when=move || error.get().is_some()>
                <p class="text-xs text-destructive">{move || error.get().unwrap_or_default()}</p>
            </Show>
        </div>
    }
}

fn format_adjustment_preview(preview_label: &str, preview: &PricingAdjustmentPreview) -> String {
    let mut label = format!(
        "{} {} -> {} {} ({} {}%)",
        preview.currency_code,
        preview.base_amount,
        preview.currency_code,
        preview.adjusted_amount,
        preview_label,
        preview.adjustment_percent
    );
    if let Some(channel_scope) = format_channel_scope_text(
        None,
        preview.channel_id.as_deref(),
        preview.channel_slug.as_deref(),
    ) {
        label.push_str(format!(" | {channel_scope}").as_str());
    }
    label
}
