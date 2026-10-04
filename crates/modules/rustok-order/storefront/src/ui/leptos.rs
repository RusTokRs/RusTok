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
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let locale_ref = locale.as_deref();

    let order_id = route_context
        .subpath
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| route_context.query_value("order_id"))
        .or_else(|| route_context.query_value("id"))
        .unwrap_or("pending")
        .to_string();

    let order_status = route_context
        .query_value("status")
        .unwrap_or("PROCESSING")
        .to_string();

    let result = OrderCheckoutResultData {
        order_id,
        order_status,
    };
    let labels = OrderCheckoutResultLabels {
        badge: t(locale_ref, "order.checkout.badge", "Order"),
        module_ownership: t(
            locale_ref,
            "order.checkout.moduleOwnership",
            "Order status and checkout completion stay in order-owned UI.",
        ),
        order_status_label: t(locale_ref, "order.checkout.orderStatus", "Order status"),
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
    let order_id = view_model.order_id.clone();
    let is_placeholder = order_id == "pending" || order_id.is_empty();

    view! {
        <article class="mt-6 rounded-3xl border border-border bg-card p-6 sm:p-8 shadow-xs">
            <div class="flex flex-wrap items-center justify-between gap-4 border-b border-border/80 pb-5">
                <div>
                    <div class="text-xs font-bold uppercase tracking-[0.18em] text-primary">
                        {labels.badge}
                    </div>
                    <h3 class="mt-2 text-xl sm:text-2xl font-extrabold text-card-foreground">
                        {if is_placeholder {
                            "Order Tracking".to_string()
                        } else {
                            format!("#{order_id}")
                        }}
                    </h3>
                    <p class="mt-1 text-xs text-muted-foreground">
                        {view_model.module_ownership}
                    </p>
                </div>
                <div class="flex items-center gap-2">
                    <span class="inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-bold bg-primary/10 text-primary">
                        {view_model.order_status_label.clone()}: " " {view_model.order_status.clone()}
                    </span>
                </div>
            </div>

            <div class="mt-6">
                <div class="grid grid-cols-1 gap-3 sm:grid-cols-5">
                    {view_model.steps.into_iter().map(|step| {
                        let is_active = step.active;
                        let is_completed = step.completed;
                        let card_class = if is_active {
                            "flex flex-col p-4 rounded-2xl border border-primary bg-primary/5 ring-1 ring-primary shadow-xs"
                        } else if is_completed {
                            "flex flex-col p-4 rounded-2xl border border-border/80 bg-background/60"
                        } else {
                            "flex flex-col p-4 rounded-2xl border border-border/40 bg-muted/20 opacity-60"
                        };
                        let badge_class = if is_completed || is_active {
                            "flex h-7 w-7 items-center justify-center rounded-xl bg-primary text-primary-foreground text-xs font-bold"
                        } else {
                            "flex h-7 w-7 items-center justify-center rounded-xl bg-muted text-muted-foreground text-xs font-bold"
                        };

                        view! {
                            <div class=card_class>
                                <div class="flex items-center justify-between mb-2">
                                    <div class=badge_class>
                                        {if is_completed { "✓" } else { "●" }}
                                    </div>
                                    <span class="text-[10px] font-bold text-muted-foreground">
                                        {format!("0{}", step.step)}
                                    </span>
                                </div>
                                <div class="text-sm font-bold text-foreground">
                                    {step.title}
                                </div>
                                <div class="mt-1 text-xs text-muted-foreground">
                                    {step.description}
                                </div>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </div>

            <div class="mt-6 grid gap-4 sm:grid-cols-2">
                <div class="rounded-2xl border border-border/70 bg-background/50 p-4">
                    <div class="text-xs font-bold uppercase tracking-wider text-muted-foreground">
                        {view_model.order_status_label}
                    </div>
                    <div class="mt-2 text-base font-bold text-foreground break-all">
                        {view_model.order_status}
                    </div>
                </div>
                <div class="rounded-2xl border border-border/70 bg-background/50 p-4">
                    <div class="text-xs font-bold uppercase tracking-wider text-muted-foreground">
                        "Fulfillment & Delivery"
                    </div>
                    <div class="mt-2 text-sm text-foreground">
                        "Standard Express Logistics (1–3 business days)"
                    </div>
                </div>
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
