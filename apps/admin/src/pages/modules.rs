use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};

use crate::entities::module::{
    BuildJob, InstalledModule, MarketplaceModule, ModuleInfo, ReleaseInfo,
};
use crate::features::modules::api;
use crate::features::modules::components::ModulesList;
use crate::shared::ui::PageHeader;
use crate::{t_string, use_i18n};

#[derive(Clone)]
struct ModulesPageData {
    modules: Vec<ModuleInfo>,
    marketplace_modules: Vec<MarketplaceModule>,
    installed_modules: Vec<InstalledModule>,
    active_build: Option<BuildJob>,
    active_release: Option<ReleaseInfo>,
    build_history: Vec<BuildJob>,
}

#[component]
pub fn Modules() -> impl IntoView {
    let i18n = use_i18n();
    let token = use_token();
    let tenant = use_tenant();

    let modules_resource = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            let modules = api::fetch_modules(token_value.clone(), tenant_value.clone()).await?;
            let marketplace_modules = api::fetch_marketplace_modules(
                token_value.clone(),
                tenant_value.clone(),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
            let installed_modules =
                api::fetch_installed_modules(token_value.clone(), tenant_value.clone()).await?;
            let active_build =
                api::fetch_active_build(token_value.clone(), tenant_value.clone()).await?;
            let active_release =
                api::fetch_active_release(token_value.clone(), tenant_value.clone()).await?;
            let build_history = api::fetch_build_history(token_value, tenant_value, 10, 0).await?;

            Ok::<ModulesPageData, _>(ModulesPageData {
                modules,
                marketplace_modules,
                installed_modules,
                active_build,
                active_release,
                build_history,
            })
        },
    );

    view! {
        <section class="px-10 py-8">
            <PageHeader
                title=t_string!(i18n, modules.title)
                eyebrow=t_string!(i18n, modules.eyebrow).to_string()
                subtitle=t_string!(i18n, modules.subtitle).to_string()
            />

            <Suspense
                fallback=move || view! {
                    <div class="space-y-4">
                        <div class="grid gap-4 xl:grid-cols-[minmax(0,1.4fr)_repeat(3,minmax(0,0.8fr))]">
                            <div class="h-40 animate-pulse rounded-xl bg-muted xl:col-span-2"></div>
                            <div class="h-40 animate-pulse rounded-xl bg-muted"></div>
                            <div class="h-40 animate-pulse rounded-xl bg-muted"></div>
                            <div class="h-40 animate-pulse rounded-xl bg-muted"></div>
                        </div>
                        <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                            {(0..6)
                                .map(|_| {
                                    view! { <div class="h-40 animate-pulse rounded-xl bg-muted"></div> }
                                })
                                .collect_view()}
                        </div>
                    </div>
                }
            >
                {move || {
                    modules_resource.get().map(|result| {
                        match result {
                            Ok(data) => {
                                view! {
                                    <ModulesList
                                        admin_surface="leptos-admin".to_string()
                                        modules=data.modules
                                        marketplace_modules=data.marketplace_modules
                                        installed_modules=data.installed_modules
                                        active_build=data.active_build
                                        active_release=data.active_release
                                        build_history=data.build_history
                                    />
                                }.into_any()
                            }
                            Err(err) => {
                                view! {
                                    <div class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {format!("{}: {}", t_string!(i18n, modules.error.load), err)}
                                    </div>
                                }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>
        </section>
    }
}
