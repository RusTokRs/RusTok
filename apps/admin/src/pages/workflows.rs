use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_router::components::A;

use crate::features::workflow::{api, WorkflowList};
use crate::shared::api::ApiError;
use crate::shared::ui::PageHeader;
use crate::{t_string, use_i18n};

#[component]
pub fn Workflows() -> impl IntoView {
    let i18n = use_i18n();
    let token = use_token();
    let tenant = use_tenant();

    let workflows_resource = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_val, tenant_val)| async move {
            api::fetch_workflows(token_val, tenant_val).await
        },
    );

    view! {
        <section class="px-10 py-8">
            <div class="flex items-start justify-between">
                <PageHeader
                    title=t_string!(i18n, workflows.title)
                    eyebrow=t_string!(i18n, workflows.eyebrow).to_string()
                    subtitle=t_string!(i18n, workflows.subtitle).to_string()
                />
                <A
                    href="/workflows/new"
                    attr:class="mt-1 flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90"
                >
                    <span>"+ "</span>
                    {t_string!(i18n, workflows.new)}
                </A>
            </div>

            <div class="mt-6">
                <Suspense
                    fallback=move || view! {
                        <div class="space-y-2">
                            {(0..4)
                                .map(|_| view! { <div class="h-14 animate-pulse rounded-xl bg-muted"></div> })
                                .collect_view()}
                        </div>
                    }
                >
                    {move || {
                        workflows_resource.get().map(|result: Result<_, ApiError>| {
                            match result {
                                Ok(workflows) => view! {
                                    <WorkflowList workflows=workflows />
                                }.into_any(),
                                Err(err) => view! {
                                    <div class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {format!("{}: {}", t_string!(i18n, workflows.error.load), err)}
                                    </div>
                                }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </div>
        </section>
    }
}
