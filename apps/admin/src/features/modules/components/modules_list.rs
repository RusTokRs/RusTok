use super::module_card::module_card;
use super::module_detail_panel::module_detail_panel;
use super::module_update_card::module_update_card;
use crate::entities::module::{BuildJob, InstalledModule, MarketplaceModule, ModuleInfo, ReleaseInfo};
use leptos::prelude::*;

pub fn modules_list(
    admin_surface: String,
    modules: Vec<ModuleInfo>,
    marketplace_modules: Vec<MarketplaceModule>,
    installed_modules: Vec<InstalledModule>,
    _active_build: Option<BuildJob>,
    _active_release: Option<ReleaseInfo>,
    _build_history: Vec<BuildJob>,
) -> impl IntoView {
    let (selected_slug, set_selected_slug) = signal::<Option<String>>(None);

    view! {
        <div class="space-y-6">
            {move || selected_slug.get().map(|slug| view! {
                {module_detail_panel(admin_surface.clone(), slug, None, Signal::derive(|| false), Callback::new(move |_| set_selected_slug.set(None)))}
            })}

            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                {modules.into_iter().map(|m| {
                    let slug = m.module_slug.clone();
                    let slug_for_select = slug.clone();
                    let installed = installed_modules.iter().any(|i| i.slug == slug);
                    let catalog = marketplace_modules.iter().find(|x| x.slug == slug).cloned();
                    view! {
                        {module_card(
                            m,
                            catalog,
                            Signal::derive(|| false),
                            Signal::derive(|| false),
                            Signal::derive(move || installed),
                            Signal::derive(|| false),
                            Signal::derive(|| None),
                            Signal::derive(|| None),
                            None,
                            None,
                            Some(Callback::new(move |_| set_selected_slug.set(Some(slug_for_select.clone())))),
                            None,
                        )}
                    }
                }).collect_view()}
            </div>

            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                {marketplace_modules.into_iter().filter_map(|m| {
                    let installed = installed_modules.iter().find(|i| i.slug == m.slug)?.clone();
                    Some(view! { {module_update_card(m, installed, Signal::derive(|| false), Signal::derive(|| false), None, Callback::new(|_| {}))} })
                }).collect_view()}
            </div>
        </div>
    }
}
