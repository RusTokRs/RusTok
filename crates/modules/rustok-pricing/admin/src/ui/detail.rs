use leptos::prelude::*;
use leptos_ui_routing::UiRouteQueryWriter;
use rustok_ui_core::AdminQueryKey;

use super::editors::*;
use crate::core::*;
use crate::i18n::t;
use crate::model::*;
use crate::transport::{self, ApiError};

#[component]
pub(crate) fn PricingDetailSection(
    ui_locale: Option<String>,
    effective_locale: Option<String>,
    bootstrap: Resource<Result<PricingAdminBootstrap, ApiError>>,
    context_active_price_lists: Resource<Result<Vec<PricingPriceListOption>, ApiError>>,
    selected: Signal<Option<PricingProductDetail>>,
    applied_resolution_context: Signal<Option<PricingResolutionContext>>,
    resolution_currency: RwSignal<String>,
    resolution_region_id: RwSignal<String>,
    resolution_price_list_id: RwSignal<String>,
    resolution_channel_id: RwSignal<String>,
    resolution_channel_slug: RwSignal<String>,
    resolution_quantity: RwSignal<String>,
    context_query_writer: UiRouteQueryWriter,
    product_module_route_base: String,
    open_product: Callback<String>,
) -> impl IntoView {
    let ui_locale_for_detail = ui_locale.clone();
    let ui_locale_for_variants = ui_locale.clone();
    let ui_locale_for_empty = ui_locale.clone();
    let ui_locale_for_price_list_select = ui_locale.clone();
    let ui_locale_for_channel_select = ui_locale.clone();
    let effective_locale_for_detail = effective_locale.clone();

    let context_query_writer_for_region = context_query_writer.clone();
    let context_query_writer_for_price_list = context_query_writer.clone();
    let context_query_writer_for_channel = context_query_writer.clone();
    let context_query_writer_for_quantity = context_query_writer.clone();

    view! {
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
                                    if selected_key == UNLISTED_CHANNEL_KEY {
                                        let label = format!(
                                            "{}: {}",
                                            t(
                                                ui_locale_for_channel_select.as_deref(),
                                                "pricing.channel.unlisted",
                                                "Unlisted scope",
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
                                            view! { <option value=UNLISTED_CHANNEL_KEY>{label}</option> }
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
                                                on_saved=refresh_detail
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
                        let detail_header = build_product_detail_header_view_model(
                            ui_locale_for_detail.as_deref(),
                            effective_locale_for_detail.as_deref(),
                            &detail,
                        );
                        let summary = summarize_pricing(detail.variants.as_slice());
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
                                                <h4 class="text-base font-semibold text-card-foreground">{detail_header.title.clone()}</h4>
                                                <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", detail_header.status_badge_class)>{detail_header.status_label.clone()}</span>
                                            </div>
                                            <p class="text-sm text-muted-foreground">{detail_header.meta_line.clone()}</p>
                                            <p class="text-sm text-muted-foreground">{detail_header.seller_line.clone()}</p>
                                            <p class="text-xs text-muted-foreground">{detail_header.shipping_line.clone()}</p>
                                        </div>
                                        <div class="text-right text-xs text-muted-foreground">
                                            <p>{detail_header.created_line.clone()}</p>
                                            <p>{detail_header.published_line.clone()}</p>
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
                                            {format_variant_count_label(ui_locale_for_detail.as_deref(), detail.variants.len())}
                                        </span>
                                    </div>
                                    <div class="mt-4 space-y-3">
                                        {detail.variants.into_iter().map(|variant| {
                                            let variant_locale = ui_locale_for_variants.clone();
                                            let variant_available_channels = available_channels.clone();
                                            let variant_price_list_options = price_list_options.clone();
                                            let variant_card = build_variant_card_view_model(
                                                variant_locale.as_deref(),
                                                &variant,
                                                variant_price_list_options.as_slice(),
                                            );
                                            view! {
                                                <article class="rounded-xl border border-border p-4">
                                                    <div class="flex flex-wrap items-start justify-between gap-3">
                                                        <div class="space-y-2">
                                                            <div class="flex flex-wrap items-center gap-2">
                                                                <h5 class="font-medium text-card-foreground">{variant_card.title.clone()}</h5>
                                                                <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", variant_card.health_badge_class)>
                                                                    {variant_card.health_label.clone()}
                                                                </span>
                                                            </div>
                                                            <p class="text-sm text-muted-foreground">{variant_card.identity_line.clone()}</p>
                                                            <p class="text-xs text-muted-foreground">{variant_card.profile_line.clone()}</p>
                                                            {variant_card.effective_price_line.as_ref().map(|line| view! {
                                                                <p class="text-sm font-medium text-foreground">
                                                                    {line.clone()}
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
                                                                    on_saved=open_product
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
                                                                    on_saved=open_product
                                                                />
                                                            </div>
                                                        </div>
                                                        <div class="space-y-1 text-right text-sm text-muted-foreground">
                                                            <p>{variant_card.price_table.clone()}</p>
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

    }
}
