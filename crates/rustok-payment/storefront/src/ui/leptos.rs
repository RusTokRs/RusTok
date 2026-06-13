use leptos::prelude::*;

use crate::core::{
    build_payment_collection_card_view_model, payment_collection_action_label,
    PaymentCollectionActionLabels, PaymentCollectionCardData, PaymentCollectionCardLabels,
};

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
    on_create_payment_collection: Callback<String>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            class="inline-flex items-center justify-center rounded-full border border-border px-4 py-2 text-sm font-medium text-card-foreground transition hover:bg-muted disabled:cursor-not-allowed disabled:opacity-60"
            disabled=move || busy.get()
            on:click={
                let cart_id = cart_id.clone();
                move |_| on_create_payment_collection.run(cart_id.clone())
            }
        >
            {move || payment_collection_action_label(busy.get(), &labels)}
        </button>
    }
}
