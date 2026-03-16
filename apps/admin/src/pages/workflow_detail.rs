use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_router::components::A;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;

use crate::entities::workflow::{WorkflowDetail, WorkflowExecution};
use crate::features::workflow::{api, ExecutionHistory, StatusBadge, WorkflowStepEditor};
use crate::shared::api::ApiError;
use crate::{t_string, use_i18n};

#[derive(Params, PartialEq)]
struct WorkflowParams {
    id: Option<String>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct WorkflowPageData {
    workflow: WorkflowDetail,
    executions: Vec<WorkflowExecution>,
}

#[component]
pub fn WorkflowDetailPage() -> impl IntoView {
    let i18n = use_i18n();
    let token = use_token();
    let tenant = use_tenant();
    let params = use_params::<WorkflowParams>();

    let workflow_id = move || {
        params.with(|p| p.as_ref().ok().and_then(|p| p.id.clone()).unwrap_or_default())
    };

    let data_resource = Resource::new(
        move || (token.get(), tenant.get(), workflow_id()),
        move |(token_val, tenant_val, wf_id): (Option<String>, Option<String>, String)| async move {
            if wf_id.is_empty() {
                return Err(ApiError::Graphql("No workflow id".to_string()));
            }
            let workflow = api::fetch_workflow(token_val.clone(), tenant_val.clone(), wf_id.clone()).await?;
            let executions = api::fetch_workflow_executions(token_val, tenant_val, wf_id).await?;
            Ok::<_, ApiError>(workflow.map(|w| WorkflowPageData { workflow: w, executions }))
        },
    );

    let token_sig = token.clone();
    let tenant_sig = tenant.clone();

    view! {
        <section class="px-10 py-8">
            <div class="mb-4">
                <A href="/workflows" attr:class="text-sm text-muted-foreground hover:text-foreground">
                    "← " {t_string!(i18n, workflows.back)}
                </A>
            </div>

            <Suspense
                fallback=move || view! {
                    <div class="space-y-4">
                        <div class="h-16 animate-pulse rounded-xl bg-muted"></div>
                        <div class="h-48 animate-pulse rounded-xl bg-muted"></div>
                    </div>
                }
            >
                {move || {
                    data_resource.get().map(|result: Result<Option<WorkflowPageData>, ApiError>| {
                        let tok = token_sig.get();
                        let ts = tenant_sig.get();

                        match result {
                            Err(err) => view! {
                                <div class="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                    {err.to_string()}
                                </div>
                            }.into_any(),
                            Ok(None) => view! {
                                <div class="rounded-lg border border-border px-4 py-12 text-center text-sm text-muted-foreground">
                                    {t_string!(i18n, workflows.not_found)}
                                </div>
                            }.into_any(),
                            Ok(Some(data)) => {
                                let wf = data.workflow.clone();
                                let wf_id = wf.id.clone();
                                let steps = wf.steps.clone();
                                let executions = data.executions.clone();

                                view! {
                                    <div class="space-y-6">
                                        // Header
                                        <div class="flex items-start justify-between">
                                            <div>
                                                <h1 class="text-2xl font-bold text-foreground">{wf.name.clone()}</h1>
                                                {wf.description.clone().map(|d| view! {
                                                    <p class="mt-1 text-sm text-muted-foreground">{d}</p>
                                                })}
                                            </div>
                                            <div class="flex items-center gap-2">
                                                <StatusBadge status=wf.status.clone() />
                                                <A
                                                    href=format!("/workflows/{}/edit", wf.id)
                                                    attr:class="rounded-lg border border-border px-3 py-1.5 text-sm font-medium hover:bg-muted"
                                                >
                                                    {t_string!(i18n, workflows.edit)}
                                                </A>
                                            </div>
                                        </div>

                                        // Stats row
                                        <div class="grid grid-cols-3 gap-4">
                                            <div class="rounded-xl border border-border bg-card p-4">
                                                <p class="text-xs text-muted-foreground">"Trigger"</p>
                                                <p class="mt-1 font-mono text-sm">
                                                    {wf.trigger_config.get("type")
                                                        .and_then(|v: &serde_json::Value| v.as_str())
                                                        .unwrap_or("unknown")
                                                        .to_string()}
                                                </p>
                                            </div>
                                            <div class="rounded-xl border border-border bg-card p-4">
                                                <p class="text-xs text-muted-foreground">"Steps"</p>
                                                <p class="mt-1 text-sm font-semibold">{steps.len()}</p>
                                            </div>
                                            <div class="rounded-xl border border-border bg-card p-4">
                                                <p class="text-xs text-muted-foreground">"Failures"</p>
                                                <p class="mt-1 text-sm font-semibold">{wf.failure_count}</p>
                                            </div>
                                        </div>

                                        // Steps editor
                                        <div>
                                            <h2 class="mb-3 text-lg font-semibold">{t_string!(i18n, workflows.steps)}</h2>
                                            <WorkflowStepEditor
                                                workflow_id=wf_id.clone()
                                                steps=steps
                                                token=tok.clone()
                                                tenant_slug=ts.clone()
                                                on_change=Callback::new(|_| {})
                                            />
                                        </div>

                                        // Execution history
                                        <div>
                                            <h2 class="mb-3 text-lg font-semibold">{t_string!(i18n, workflows.executions)}</h2>
                                            <ExecutionHistory executions=executions />
                                        </div>
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
