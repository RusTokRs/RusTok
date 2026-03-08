use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};

use crate::features::modules::api;
use crate::features::modules::components::ModulesList;
use crate::shared::ui::PageHeader;
use crate::{t_string, use_i18n};

#[component]
pub fn Modules() -> impl IntoView {
    let i18n = use_i18n();
    let token = use_token();
    let tenant = use_tenant();

    let modules_resource = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            api::fetch_modules(token_value, tenant_value).await
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
                    <div class="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
                        {(0..6)
                            .map(|_| {
                                view! { <div class="h-40 animate-pulse rounded-xl bg-muted"></div> }
                            })
                            .collect_view()}
                    </div>
                }
            >
                {move || {
                    modules_resource.get().map(|result| {
                        match result {
                            Ok(modules) => {
                                view! { <ModulesList modules=modules /> }.into_any()
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
