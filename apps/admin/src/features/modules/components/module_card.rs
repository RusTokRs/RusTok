use leptos::prelude::*;

use crate::entities::module::{MarketplaceModule, ModuleInfo};

pub fn module_card(
    module: ModuleInfo,
    catalog_module: Option<MarketplaceModule>,
    _tenant_loading: Signal<bool>,
    _platform_loading: Signal<bool>,
    _platform_installed: Signal<bool>,
    _platform_busy: Signal<bool>,
    _platform_version: Signal<Option<String>>,
    _recommended_version: Signal<Option<String>>,
    _on_toggle: Option<Callback<(String, bool)>>,
    on_install: Option<Callback<(String, String)>>,
    on_inspect: Option<Callback<String>>,
    on_uninstall: Option<Callback<String>>,
) -> impl IntoView {
    let slug = module.module_slug.clone();
    let version = module.version.clone();
    let description = module.description.clone();
    let name = module.name.clone();

    view! {
        <div class="rounded-xl border border-border bg-card p-4 space-y-3">
            <div>
                <h3 class="text-sm font-semibold">{name}</h3>
                <p class="text-sm text-muted-foreground">{description}</p>
            </div>
            <div class="flex gap-2">
                {on_inspect.clone().map(|cb| { let slug_inspect = slug.clone(); view! { <button class="text-xs underline" on:click=move |_| cb.run(slug_inspect.clone())>"Inspect"</button> } })}
                {on_install.clone().map(|cb| { let slug_install = slug.clone(); let version_install = version.clone(); view! { <button class="text-xs underline" on:click=move |_| cb.run((slug_install.clone(), version_install.clone()))>"Install"</button> } })}
                {on_uninstall.clone().map(|cb| { let slug_uninstall = slug.clone(); view! { <button class="text-xs underline" on:click=move |_| cb.run(slug_uninstall.clone())>"Uninstall"</button> } })}
            </div>
            {catalog_module.map(|m| view! { <p class="text-xs text-muted-foreground">{format!("Publisher: {}", m.publisher.unwrap_or_default())}</p> })}
        </div>
    }
}
