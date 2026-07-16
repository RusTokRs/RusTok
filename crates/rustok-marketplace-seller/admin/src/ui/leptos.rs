use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    build_marketplace_seller_admin_shell, selected_transport_profile,
};
use crate::i18n::normalize_admin_locale;

#[component]
pub fn MarketplaceSellerAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = normalize_admin_locale(route_context.locale.as_deref());
    let profile = selected_transport_profile(None);
    let shell = build_marketplace_seller_admin_shell(Some(locale), profile);

    view! {
        <section class="marketplace-seller-admin" data-transport-profile=shell.transport_profile>
            <header>
                <p class="marketplace-seller-admin__family">"Marketplace Family"</p>
                <h1>{shell.title}</h1>
                <p>{shell.subtitle}</p>
            </header>
            <div class="marketplace-seller-admin__empty" role="status">
                {shell.empty_state}
            </div>
        </section>
    }
}
