use leptos::prelude::*;

#[component]
pub fn FulfillmentShippingHandoffNotice(message: String) -> impl IntoView {
    view! {
        <div class="mt-6 rounded-2xl border border-dashed border-border px-4 py-3 text-sm text-muted-foreground">
            {message}
        </div>
    }
}
