use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    PaymentCollectionActionLabels, PaymentCollectionCardData, PaymentCollectionCardLabels,
    build_payment_collection_card_view_model, payment_collection_action_label,
};
use crate::i18n::t;
use crate::transport::{PaymentCollectionCreateRequest, build_payment_collection_create_request};

#[component]
pub fn PaymentView() -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let locale = locale.as_deref();
    let payment_collection = None;
    let labels = PaymentCollectionCardLabels {
        badge: t(locale, "payment.collection.badge", "Payment"),
        module_ownership: t(
            locale,
            "payment.collection.moduleOwnership",
            "Payment collection presentation stays in payment-owned UI.",
        ),
        empty_id: t(
            locale,
            "payment.collection.emptyId",
            "No payment collection",
        ),
        empty_status: t(locale, "payment.collection.emptyStatus", "Not started"),
    };

    view! {
        <PaymentCollectionCard
            payment_collection
            labels
        />
    }
}

#[component]
pub fn PaymentCollectionCard(
    payment_collection: Option<PaymentCollectionCardData>,
    labels: PaymentCollectionCardLabels,
) -> impl IntoView {
    let view_model = build_payment_collection_card_view_model(payment_collection, &labels);

    view! {
        <article class="rounded-2xl border border-dashed border-border p-5">
            <div class="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                {labels.badge}
            </div>
            <p class="mt-2 text-sm text-muted-foreground">
                {labels.module_ownership}
            </p>
            <div class="mt-4 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                {format!("{} · {}", view_model.collection_id, view_model.status)}
            </div>
        </article>
    }
}

#[component]
pub fn PaymentCollectionActionButton(
    cart_id: String,
    busy: ReadSignal<bool>,
    labels: PaymentCollectionActionLabels,
    on_create_payment_collection: Callback<PaymentCollectionCreateRequest>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            class="inline-flex items-center justify-center rounded-full border border-border px-4 py-2 text-sm font-medium text-card-foreground transition hover:bg-muted disabled:cursor-not-allowed disabled:opacity-60"
            disabled=move || busy.get()
            on:click={
                let cart_id = cart_id.clone();
                move |_| {
                    on_create_payment_collection.run(build_payment_collection_create_request(cart_id.clone()))
                }
            }
        >
            {move || payment_collection_action_label(busy.get(), &labels)}
        </button>
    }
}
