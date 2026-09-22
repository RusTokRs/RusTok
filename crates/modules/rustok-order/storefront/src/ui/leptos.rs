use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    OrderCheckoutActionLabels, OrderCheckoutResultData, OrderCheckoutResultLabels,
    build_order_checkout_result_view_model, order_checkout_action_label,
};
use crate::i18n::t;
use crate::transport::{CompleteCheckoutRequest, build_complete_checkout_request};

#[component]
pub fn OrderView() -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let locale = locale.as_deref();
    let result = OrderCheckoutResultData {
        order_id: "pending".to_string(),
        order_status: "not_started".to_string(),
    };
    let labels = OrderCheckoutResultLabels {
        badge: t(locale, "order.checkout.badge", "Order"),
        module_ownership: t(
            locale,
            "order.checkout.moduleOwnership",
            "Order status and checkout completion stay in order-owned UI.",
        ),
        order_status_label: t(locale, "order.checkout.orderStatus", "Order status"),
    };

    view! {
        <OrderCheckoutResultCard
            result
            labels
        />
    }
}

#[component]
pub fn OrderCheckoutResultCard(
    result: OrderCheckoutResultData,
    labels: OrderCheckoutResultLabels,
) -> impl IntoView {
    let view_model = build_order_checkout_result_view_model(result, &labels);

    view! {
        <article class="mt-6 rounded-2xl border border-primary/30 bg-primary/5 p-5">
            <div class="text-xs font-medium uppercase tracking-[0.18em] text-primary">
                {labels.badge}
            </div>
            <h4 class="mt-2 text-base font-semibold text-card-foreground">{view_model.order_id}</h4>
            <p class="mt-2 text-sm text-muted-foreground">
                {view_model.module_ownership}
            </p>
            <div class="mt-4 grid gap-3 md:grid-cols-2">
                <article class="rounded-2xl border border-border bg-card p-4">
                    <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                        {view_model.order_status_label}
                    </div>
                    <div class="mt-2 text-lg font-semibold text-card-foreground break-all">
                        {view_model.order_status}
                    </div>
                </article>
            </div>
        </article>
    }
}

#[component]
pub fn OrderCheckoutCompleteButton(
    cart_id: String,
    busy: ReadSignal<bool>,
    labels: OrderCheckoutActionLabels,
    on_complete_checkout: Callback<CompleteCheckoutRequest>,
) -> impl IntoView {
    // Build once per component instance so network retries reuse the exact same
    // checkout idempotency key instead of opening a second operation.
    let request = build_complete_checkout_request(cart_id);
    view! {
        <button
            type="button"
            class="inline-flex items-center justify-center rounded-full border border-primary bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60 md:col-span-2"
            disabled=move || busy.get()
            on:click={
                let request = request.clone();
                move |_| on_complete_checkout.run(request.clone())
            }
        >
            {move || order_checkout_action_label(busy.get(), &labels)}
        </button>
    }
}
