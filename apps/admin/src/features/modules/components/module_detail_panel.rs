use leptos::prelude::*;

use crate::entities::module::MarketplaceModule;

pub fn module_detail_panel(
    _admin_surface: String,
    selected_slug: String,
    module: Option<MarketplaceModule>,
    _loading: Signal<bool>,
    on_close: Callback<()>,
) -> impl IntoView {
    let title = module
        .as_ref()
        .map(|m| m.name.clone())
        .unwrap_or(selected_slug);

    view! {
        <div class="rounded-xl border border-border bg-card p-4">
            <div class="flex items-center justify-between">
                <h3 class="font-semibold">{title}</h3>
                <button class="text-xs underline" on:click=move |_| on_close.run(())>"Close"</button>
            </div>
        </div>
    }
}
