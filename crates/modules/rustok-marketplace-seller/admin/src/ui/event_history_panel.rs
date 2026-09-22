use leptos::prelude::*;

use crate::event_timeline::MarketplaceSellerEventTimeline;
use crate::transport::{
    MarketplaceSellerAdminTransportContext, load_marketplace_seller_event_history,
};

#[component]
pub fn MarketplaceSellerEventHistoryPanel(
    context: MarketplaceSellerAdminTransportContext,
    seller_id: String,
    #[prop(default = 100)] limit: u64,
) -> impl IntoView {
    let history = LocalResource::new(move || {
        let context = context.clone();
        let seller_id = seller_id.clone();
        async move { load_marketplace_seller_event_history(context, seller_id, limit).await }
    });

    view! {
        <Suspense fallback=move || view! {
            <div class="rounded border p-4 text-sm text-muted-foreground">
                "Loading seller timeline..."
            </div>
        }>
            {move || history.get().map(|result| match result {
                Ok(history) => view! { <MarketplaceSellerEventTimeline history=history/> }.into_any(),
                Err(error) => view! {
                    <div class="rounded border border-destructive p-4 text-sm text-destructive" role="alert">
                        <p class="font-semibold">"Seller timeline unavailable"</p>
                        <p class="mt-1">{error.to_string()}</p>
                    </div>
                }
                    .into_any(),
            })}
        </Suspense>
    }
}
