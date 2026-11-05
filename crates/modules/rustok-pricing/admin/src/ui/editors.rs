use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::core::*;
use crate::i18n::t;
use crate::model::*;
use crate::transport;

#[component]
pub(crate) fn StatCard(title: String, value: String, hint: String) -> impl IntoView {
    view! {
        <div class="rounded-2xl border border-border bg-background p-4">
            <p class="text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground">{title}</p>
            <p class="mt-3 text-2xl font-semibold text-card-foreground">{value}</p>
            <p class="mt-2 text-xs text-muted-foreground">{hint}</p>
        </div>
    }
}

#[component]
pub(crate) fn VariantPriceEditors(
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
            let title = format_variant_price_editor_title(locale.as_deref(), &price);
            view! {
                <VariantPriceEditor
                    locale=locale.clone()
                    product_id=product_id.clone()
                    variant_id=variant.id.clone()
                    available_channels=available_channels.clone()
                    draft=price_draft_from_price(price)
                    title=title
                    on_saved=on_saved
                />
            }
        })
        .collect_view();
    let add_currency = default_variant_price_editor_currency(&variant, default_currency);

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
                draft=empty_price_draft(add_currency, selected_price_list_id)
                title=add_price_title
                on_saved=on_saved
            />
        </div>
    }
}

#[component]
pub(crate) fn VariantDiscountEditor(
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
    let available_channels_for_unlisted = available_channels.clone();
    let locale_for_preview = locale.clone();
    let locale_for_apply = locale.clone();
    let locale_for_button = locale.clone();
    let locale_for_apply_button = locale.clone();
    let locale_for_unlisted_label = locale.clone();
    let unlisted_option_label = Memo::new(move |_| {
        unlisted_channel_option_label(
            locale_for_unlisted_label.as_deref(),
            channel_id.get().as_str(),
            channel_slug.get().as_str(),
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
                            available_channels_for_unlisted.as_slice(),
                        ) == UNLISTED_CHANNEL_KEY
                    }>
                        <option value=UNLISTED_CHANNEL_KEY>
                            {move || unlisted_option_label.get()}
                        </option>
                    </Show>
                </select>
                <button
                    type="button"
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=move |_| {
                        let payload = build_discount_draft(
                            currency_code.get_untracked(),
                            discount_percent.get_untracked(),
                            preview_selected_price_list_id.clone(),
                            channel_id.get_untracked(),
                            channel_slug.get_untracked(),
                        );
                        let variant_id = preview_variant_id.clone();
                        let preview_error_label = t(locale_for_preview.as_deref(), "pricing.adjustment.previewError", "Failed to preview discount");
                        set_busy.set(true);
                        set_error.set(None);
                        spawn_local(async move {
                            match transport::preview_variant_discount(variant_id, payload).await {
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
                        let payload = build_discount_draft(
                            currency_code.get_untracked(),
                            discount_percent.get_untracked(),
                            apply_selected_price_list_id.clone(),
                            channel_id.get_untracked(),
                            channel_slug.get_untracked(),
                        );
                        let variant_id = apply_variant_id.clone();
                        let product_id = product_id.clone();
                        let on_saved = on_saved;
                        let apply_error_label = t(locale_for_apply.as_deref(), "pricing.adjustment.applyError", "Failed to apply discount");
                        set_busy.set(true);
                        set_error.set(None);
                        spawn_local(async move {
                            match transport::apply_variant_discount(variant_id, payload).await {
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
pub(crate) fn PriceListRuleEditor(
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
    let available_channels_for_unlisted = available_channels.clone();
    let locale_for_unlisted_label = locale.clone();
    let unlisted_option_label = Memo::new(move |_| {
        unlisted_channel_option_label(
            locale_for_unlisted_label.as_deref(),
            channel_id.get().as_str(),
            channel_slug.get().as_str(),
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
                        let payload = build_price_list_rule_draft(adjustment_percent.get_untracked());
                        let price_list_id = save_price_list_id.clone();
                        let on_saved = on_saved;
                        let save_error_label = t(locale_for_save.as_deref(), "pricing.rule.saveError", "Failed to save price-list rule");
                        set_busy.set(true);
                        set_error.set(None);
                        spawn_local(async move {
                            match transport::update_price_list_rule(price_list_id, payload).await {
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
                        let payload = clear_price_list_rule_draft();
                        let price_list_id = clear_price_list_id.clone();
                        let on_saved = on_saved;
                        let save_error_label = t(locale_for_clear.as_deref(), "pricing.rule.saveError", "Failed to save price-list rule");
                        set_busy.set(true);
                        set_error.set(None);
                        spawn_local(async move {
                            match transport::update_price_list_rule(price_list_id, payload).await {
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
                            available_channels_for_unlisted.as_slice(),
                        ) == UNLISTED_CHANNEL_KEY
                    }>
                        <option value=UNLISTED_CHANNEL_KEY>
                            {move || unlisted_option_label.get()}
                        </option>
                    </Show>
                </select>
                <button
                    type="button"
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=move |_| {
                        let payload = build_price_list_scope_draft(
                            channel_id.get_untracked(),
                            channel_slug.get_untracked(),
                        );
                        let price_list_id = save_scope_price_list_id.clone();
                        let on_saved = on_saved;
                        let save_error_label = t(locale_for_scope_save.as_deref(), "pricing.rule.scopeSaveError", "Failed to save price-list scope");
                        set_busy.set(true);
                        set_error.set(None);
                        spawn_local(async move {
                            match transport::update_price_list_scope(price_list_id, payload).await {
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
pub(crate) fn VariantPriceEditor(
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
    let available_channels_for_unlisted = available_channels.clone();
    let locale_for_unlisted_label = locale.clone();
    let unlisted_option_label = Memo::new(move |_| {
        unlisted_channel_option_label(
            locale_for_unlisted_label.as_deref(),
            channel_id.get().as_str(),
            channel_slug.get().as_str(),
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
                            available_channels_for_unlisted.as_slice(),
                        ) == UNLISTED_CHANNEL_KEY
                    }>
                        <option value=UNLISTED_CHANNEL_KEY>
                            {move || unlisted_option_label.get()}
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
                        let payload = build_price_draft(PriceDraftForm {
                            currency_code: currency_code.get_untracked(),
                            amount: amount.get_untracked(),
                            compare_at_amount: compare_at_amount.get_untracked(),
                            price_list_id: price_list_id.clone(),
                            channel_id: channel_id.get_untracked(),
                            channel_slug: channel_slug.get_untracked(),
                            min_quantity: min_quantity.get_untracked(),
                            max_quantity: max_quantity.get_untracked(),
                        });
                        let on_saved = on_saved;
                        let save_error_label = t(locale_for_save.as_deref(), "pricing.edit.saveError", "Failed to save price");
                        set_busy.set(true);
                        set_error.set(None);
                        spawn_local(async move {
                            match transport::update_variant_price(variant_id, payload).await {
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
