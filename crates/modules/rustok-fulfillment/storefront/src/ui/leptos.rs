use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    SelectShippingOptionRequest, ShippingSelectionLabels, build_select_shipping_option_request,
    delivery_group_key, format_line_items_label, format_provider_label,
    format_shipping_option_price, is_shipping_option_selected, selection_action_label,
};
use crate::model::{StorefrontDeliveryGroup, StorefrontShippingOption};

#[component]
pub fn FulfillmentView() -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let locale = locale.as_deref();
    let busy = RwSignal::new(false).read_only();
    let on_select_shipping_option = Callback::new(|_: SelectShippingOptionRequest| {});
    let delivery_groups = Vec::<StorefrontDeliveryGroup>::new();
    let labels = ShippingSelectionLabels::resolve(locale);

    view! {
        <FulfillmentShippingSelectionPanel
            delivery_groups
            labels
            busy
            on_select_shipping_option
        />
    }
}

#[component]
pub fn FulfillmentShippingHandoffNotice(message: String) -> impl IntoView {
    view! {
        <div class="mt-6 rounded-2xl border border-dashed border-border px-4 py-3 text-sm text-muted-foreground">
            {message}
        </div>
    }
}

#[component]
pub fn FulfillmentShippingSelectionPanel(
    delivery_groups: Vec<StorefrontDeliveryGroup>,
    labels: ShippingSelectionLabels,
    busy: ReadSignal<bool>,
    on_select_shipping_option: Callback<SelectShippingOptionRequest>,
) -> impl IntoView {
    if delivery_groups.is_empty() {
        return view! {
            <section class="mt-6 rounded-3xl border border-dashed border-border p-6">
                <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{labels.badge}</div>
                <h4 class="mt-2 text-lg font-semibold text-card-foreground">{labels.title}</h4>
                <p class="mt-2 text-sm text-muted-foreground">{labels.empty}</p>
            </section>
        }
        .into_any();
    }

    view! {
        <section class="mt-6 rounded-3xl border border-border bg-card p-6">
            <div class="space-y-2">
                <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{labels.badge.clone()}</div>
                <h4 class="text-lg font-semibold text-card-foreground">{labels.title.clone()}</h4>
                <p class="text-sm text-muted-foreground">{labels.subtitle.clone()}</p>
            </div>
            <div class="mt-5 grid gap-4">
                <For
                    each=move || delivery_groups.clone()
                    key=|group| delivery_group_key(group)
                    children=move |group| {
                        view! {
                            <DeliveryGroupCard
                                group
                                labels=labels.clone()
                                busy
                                on_select_shipping_option
                            />
                        }
                    }
                />
            </div>
        </section>
    }
    .into_any()
}

#[component]
fn DeliveryGroupCard(
    group: StorefrontDeliveryGroup,
    labels: ShippingSelectionLabels,
    busy: ReadSignal<bool>,
    on_select_shipping_option: Callback<SelectShippingOptionRequest>,
) -> impl IntoView {
    let selected_id = group.selected_shipping_option_id.clone();
    let group_for_clear = group.clone();
    let clear_label = labels.no_selection_label.clone();
    let select_clear = move |_| {
        on_select_shipping_option.run(build_select_shipping_option_request(&group_for_clear, None));
    };
    let stored_group = StoredValue::new(group.clone());
    let group_label = labels.group_label.clone();
    let line_items_label = format_line_items_label(&labels.line_items_label, group.line_item_count);
    let clear_action_label = selection_action_label(selected_id.is_none(), &labels).to_string();
    let option_labels = labels.clone();

    view! {
        <article class="rounded-2xl border border-border bg-background p-5">
            <div class="flex flex-wrap items-start justify-between gap-3">
                <div>
                    <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{group_label}</div>
                    <h5 class="mt-1 text-base font-semibold text-card-foreground">{group.shipping_profile_slug.clone()}</h5>
                </div>
                <div class="rounded-full border border-border px-3 py-1 text-xs text-muted-foreground">
                    {line_items_label}
                </div>
            </div>
            <div class="mt-4 grid gap-3">
                <button
                    type="button"
                    class="flex items-center justify-between rounded-xl border border-border px-4 py-3 text-left text-sm transition hover:bg-muted disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=select_clear
                >
                    <span class="font-medium text-card-foreground">{clear_label}</span>
                    <span class="text-xs text-muted-foreground">{clear_action_label}</span>
                </button>
                <For
                    each=move || stored_group.with_value(|g| g.available_shipping_options.clone())
                    key=|option| option.id.clone()
                    children=move |option| {
                        let group_val = stored_group.get_value();
                        let selected_id_opt = group_val.selected_shipping_option_id.clone();
                        view! {
                            <ShippingOptionButton
                                group=group_val
                                option
                                selected_shipping_option_id=selected_id_opt
                                labels=option_labels.clone()
                                busy
                                on_select_shipping_option
                            />
                        }
                    }
                />
            </div>
        </article>
    }
}

#[component]
fn ShippingOptionButton(
    group: StorefrontDeliveryGroup,
    option: StorefrontShippingOption,
    selected_shipping_option_id: Option<String>,
    labels: ShippingSelectionLabels,
    busy: ReadSignal<bool>,
    on_select_shipping_option: Callback<SelectShippingOptionRequest>,
) -> impl IntoView {
    let is_selected =
        is_shipping_option_selected(selected_shipping_option_id.as_deref(), &option.id);
    let option_for_click = option.clone();
    let group_for_click = group.clone();
    let on_click = move |_| {
        on_select_shipping_option.run(build_select_shipping_option_request(
            &group_for_click,
            Some(option_for_click.id.clone()),
        ));
    };
    let provider_label = format_provider_label(&labels.provider_label, &option.provider_id);
    let price_label = format_shipping_option_price(&option.amount, &option.currency_code);
    let action_label = selection_action_label(is_selected, &labels).to_string();

    view! {
        <button
            type="button"
            class="rounded-xl border border-border px-4 py-3 text-left text-sm transition hover:bg-muted disabled:opacity-50"
            disabled=move || busy.get() || !option.active
            on:click=on_click
        >
            <div class="flex items-start justify-between gap-3">
                <div>
                    <div class="font-medium text-card-foreground">{option.name}</div>
                    <div class="mt-1 text-xs text-muted-foreground">{provider_label}</div>
                </div>
                <div class="text-right">
                    <div class="font-semibold text-card-foreground">{price_label}</div>
                    <div class="mt-1 text-xs text-muted-foreground">{action_label}</div>
                </div>
            </div>
        </button>
    }
}
