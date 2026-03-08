use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};

use crate::entities::module::ModuleInfo;
use crate::features::modules::api;
use crate::{t, t_string, use_i18n};

use super::module_card::ModuleCard;

#[component]
pub fn ModulesList(modules: Vec<ModuleInfo>) -> impl IntoView {
    let i18n = use_i18n();
    let (module_list, set_module_list) = signal(modules);
    let (loading_slug, set_loading_slug) = signal::<Option<String>>(None);
    let (error_msg, set_error_msg) = signal::<Option<String>>(None);
    let (success_msg, set_success_msg) = signal::<Option<String>>(None);

    let token = use_token();
    let tenant = use_tenant();

    let on_toggle = Callback::new(move |(slug, enabled): (String, bool)| {
        let slug_clone = slug.clone();
        set_loading_slug.set(Some(slug.clone()));
        set_error_msg.set(None);
        set_success_msg.set(None);

        let token_val = token.get();
        let tenant_val = tenant.get();

        spawn_local(async move {
            match api::toggle_module(slug_clone.clone(), enabled, token_val, tenant_val).await {
                Ok(result) => {
                    set_module_list.update(|modules| {
                        if let Some(m) = modules.iter_mut().find(|m| m.module_slug == slug_clone) {
                            m.enabled = result.enabled;
                        }
                    });
                    let status = if result.enabled {
                        t_string!(i18n, modules.toast.enabled)
                    } else {
                        t_string!(i18n, modules.toast.disabled)
                    };
                    set_success_msg.set(Some(status.to_string()));
                }
                Err(err) => {
                    set_error_msg.set(Some(format!("{}", err)));
                }
            }
            set_loading_slug.set(None);
        });
    });

    let core_modules = move || {
        module_list
            .get()
            .into_iter()
            .filter(|m| m.is_core())
            .collect::<Vec<_>>()
    };

    let optional_modules = move || {
        module_list
            .get()
            .into_iter()
            .filter(|m| !m.is_core())
            .collect::<Vec<_>>()
    };

    view! {
        <div class="space-y-8">
            {move || error_msg.get().map(|msg| view! {
                <div class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                    {msg}
                </div>
            })}

            {move || success_msg.get().map(|msg| view! {
                <div class="rounded-lg border border-emerald-500/50 bg-emerald-50 px-4 py-3 text-sm text-emerald-700 dark:bg-emerald-900/20 dark:text-emerald-400">
                    {msg}
                </div>
            })}

            // Core modules
            <div class="space-y-3">
                <div class="flex items-center gap-2">
                    <svg class="h-5 w-5 text-muted-foreground" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
                    </svg>
                    <h3 class="text-lg font-semibold text-foreground">{t!(i18n, modules.section.core)}</h3>
                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                        {t!(i18n, modules.always_active)}
                    </span>
                </div>
                <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
                    {move || core_modules().into_iter().map(|module| {
                        let slug = module.module_slug.clone();
                        let is_loading = Signal::derive(move || loading_slug.get().as_deref() == Some(&slug));
                        view! {
                            <ModuleCard
                                module=module
                                loading=is_loading
                            />
                        }
                    }).collect_view()}
                </div>
            </div>

            // Optional modules
            <div class="space-y-3">
                <div class="flex items-center gap-2">
                    <svg class="h-5 w-5 text-muted-foreground" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4" />
                    </svg>
                    <h3 class="text-lg font-semibold text-foreground">{t!(i18n, modules.section.optional)}</h3>
                </div>
                <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
                    {move || optional_modules().into_iter().map(|module| {
                        let slug = module.module_slug.clone();
                        let is_loading = Signal::derive(move || loading_slug.get().as_deref() == Some(&slug));
                        view! {
                            <ModuleCard
                                module=module
                                loading=is_loading
                                on_toggle=on_toggle
                            />
                        }
                    }).collect_view()}
                </div>
            </div>
        </div>
    }
}
