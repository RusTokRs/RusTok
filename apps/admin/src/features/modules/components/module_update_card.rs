use leptos::prelude::*;

use crate::entities::module::{InstalledModule, MarketplaceModule};

pub fn module_update_card(
    module: MarketplaceModule,
    installed_module: InstalledModule,
    _platform_loading: Signal<bool>,
    _platform_busy: Signal<bool>,
    _on_inspect: Option<Callback<String>>,
    on_upgrade: Callback<(String, String)>,
) -> impl IntoView {
    let slug = module.slug.clone();
    let target = module.latest_version.clone();
    view! {
        <div class="rounded-xl border border-border bg-card p-4 space-y-2">
            <h3 class="text-sm font-semibold">{module.name}</h3>
            <p class="text-xs text-muted-foreground">{format!("{} -> {}", installed_module.version.unwrap_or_default(), target.clone())}</p>
            <button class="text-xs underline" on:click=move |_| on_upgrade.run((slug.clone(), target.clone()))>"Upgrade"</button>
        </div>
    }
}
