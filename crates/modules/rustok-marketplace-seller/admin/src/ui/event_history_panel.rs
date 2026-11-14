use leptos::prelude::*;

use crate::core::MarketplaceSellerAdminTransportProfile;
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
    // The transport context carries no `serde` impl, so the source is the values it is rebuilt
    // from and the fetcher assembles it. See
    // `DECISIONS/2026-11-13-standalone-admin-editor-delivery.md`.
    let history = Resource::new(
        move || {
            (
                context.profile,
                context.access_token.clone(),
                context.tenant_slug.clone(),
                seller_id.clone(),
            )
        },
        move |(profile, access_token, tenant_slug, seller_id)| async move {
            load_marketplace_seller_event_history(
                transport_context_for(profile, access_token, tenant_slug),
                seller_id,
                limit,
            )
            .await
        },
    );

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

/// Rebuilds the context inside the fetcher from the values the resource source carries, because
/// the context itself is not `serde`.
fn transport_context_for(
    profile: MarketplaceSellerAdminTransportProfile,
    access_token: Option<String>,
    tenant_slug: Option<String>,
) -> MarketplaceSellerAdminTransportContext {
    match profile {
        MarketplaceSellerAdminTransportProfile::Native => {
            MarketplaceSellerAdminTransportContext::native()
        }
        MarketplaceSellerAdminTransportProfile::Graphql => {
            MarketplaceSellerAdminTransportContext::graphql(access_token, tenant_slug)
        }
    }
}
