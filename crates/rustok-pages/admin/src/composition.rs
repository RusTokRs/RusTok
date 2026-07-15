use crate::builder::{self, PagesBuilderFacade, PagesBuilderSaveSnapshot};
use crate::core;
use crate::i18n::t;
use crate::model::{PageBuilderScenarioReleaseStatus, PageDetail};
use crate::transport;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::use_route_query_value;
use rustok_page_builder::runtime_context::{
    generate_page_builder_runtime_example, PageBuilderRuntimeExampleRequest,
};
use rustok_page_builder::runtime_scenario_release::RuntimeScenarioReleaseBaseline;
use rustok_page_builder::{RuntimeContextExamplePolicy, RuntimeContextScenario};
use rustok_page_builder_admin::{
    PageBuilderAdmin, PageBuilderAdminFacade, PageBuilderAdminHostContext,
};
use rustok_ui_core::{AdminQueryKey, UiRouteContext};
use serde_json::{json, Value};
use std::sync::Arc;

#[component]
pub fn PagesAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_page_query = use_route_query_value(AdminQueryKey::PageId.as_str());
    let token = use_token();
    let tenant = use_tenant();
    let resource_token = token.clone();
    let resource_tenant = tenant.clone();
    let default_locale = route_context.locale.clone().unwrap_or_default();
    let loading_label = t(
        route_context.locale.as_deref(),
        "pages.builder.loading",
        "Loading visual editor...",
    );
    let missing_label = t(
        route_context.locale.as_deref(),
        "pages.builder.missing",
        "Select an existing page to open the Fly visual editor.",
    );
    let load_error_label = t(
        route_context.locale.as_deref(),
        "pages.builder.loadError",
        "Failed to load the selected page for visual editing",
    );

    let builder_resource = LocalResource::new(move || {
        let page_id = selected_page_query.get();
        let token = resource_token.get();
        let tenant = resource_tenant.get();
        async move {
            let Some(page_id) = page_id.filter(|page_id| core::optional_ui_text(page_id).is_some())
            else {
                return Ok(None);
            };
            let Some(page) =
                transport::fetch_page(token.clone(), tenant.clone(), page_id.clone()).await?
            else {
                return Ok(None);
            };
            let baseline = transport::fetch_page_builder_scenario_baseline(
                token.clone(),
                tenant.clone(),
                page_id.clone(),
            )
            .await?;
            let release_status =
                transport::fetch_page_builder_scenario_release_status(token, tenant, page_id)
                    .await?;
            Ok(Some((page, baseline, release_status)))
        }
    });

    view! {
        <div class="space-y-6">
            <section class="rounded-2xl border border-border bg-card p-4 shadow-sm">
                <Suspense fallback=move || view! {
                    <div class="h-48 animate-pulse rounded-xl bg-muted" aria-label=loading_label.clone()></div>
                }>
                    {move || {
                        builder_resource.get().map(|result| match result {
                            Ok(Some((page, baseline, release_status))) => view! {
                                <PagesFlyBuilder
                                    page
                                    baseline
                                    release_status
                                    token=token.clone()
                                    tenant=tenant.clone()
                                    default_locale=default_locale.clone()
                                />
                            }.into_any(),
                            Ok(None) => view! {
                                <div class="rounded-xl border border-dashed border-border px-5 py-8 text-sm text-muted-foreground" role="status">
                                    {missing_label.clone()}
                                </div>
                            }.into_any(),
                            Err(error) => view! {
                                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">
                                    {format!("{load_error_label}: {error}")}
                                </div>
                            }.into_any(),
                        })
                    }}
                </Suspense>
            </section>

            <crate::ui::leptos::PagesAdmin />
        </div>
    }
}

#[component]
fn PagesFlyBuilder(
    page: PageDetail,
    #[prop(optional)] baseline: Option<RuntimeScenarioReleaseBaseline>,
    release_status: PageBuilderScenarioReleaseStatus,
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    default_locale: String,
) -> impl IntoView {
    let seed = core::edit_form_seed_from_page(&page, &default_locale);
    let revision_id = builder::page_revision(&page);
    let fallback_page_id = page.id.clone();
    let project_data =
        serde_json::from_str::<Value>(&seed.project_data_text).unwrap_or_else(|_| {
            json!({
                "pages": [{
                    "id": fallback_page_id,
                    "component": { "id": "root", "type": "wrapper" }
                }]
            })
        });
    let generated_context =
        generate_page_builder_runtime_example(PageBuilderRuntimeExampleRequest {
            project_data: project_data.clone(),
            policy: RuntimeContextExamplePolicy::default(),
        })
        .ok()
        .map(|response| response.example.input_context)
        .unwrap_or_else(|| json!({}));
    let scenarios = Arc::new(vec![
        RuntimeContextScenario::new("empty", "Empty", json!({})),
        RuntimeContextScenario::new("generated", "Generated example", generated_context.clone()),
    ]);
    let controller =
        builder::controller_from_project(&page.id, &revision_id, &seed.project_data_text);

    match controller {
        Ok(controller) => {
            let page_id = page.id.clone();
            let snapshot_default_locale = default_locale.clone();
            let facade_token = token.clone();
            let facade_tenant = tenant.clone();
            let facade: Arc<dyn PageBuilderAdminFacade> = Arc::new(PagesBuilderFacade::new(
                move || PagesBuilderSaveSnapshot {
                    token: facade_token.get_untracked(),
                    tenant_slug: facade_tenant.get_untracked(),
                    page_id: page_id.clone(),
                    default_locale: snapshot_default_locale.clone(),
                },
                |_page, _project_data| {},
            ));

            let persistence_error = RwSignal::new(None::<String>);
            let server_status = RwSignal::new(release_status);
            let baseline_page_id = page.id.clone();
            let baseline_token = token.clone();
            let baseline_tenant = tenant.clone();
            let on_baseline = Callback::new(
                move |baseline: Option<RuntimeScenarioReleaseBaseline>| {
                    let page_id = baseline_page_id.clone();
                    let token = baseline_token.get_untracked();
                    let tenant = baseline_tenant.get_untracked();
                    let expected_baseline_hash =
                        server_status.get_untracked().baseline_hash.clone();
                    spawn_local(async move {
                        let write_result = match baseline {
                            Some(baseline) => transport::save_page_builder_scenario_baseline(
                                token.clone(),
                                tenant.clone(),
                                page_id.clone(),
                                baseline,
                                expected_baseline_hash,
                            )
                            .await
                            .map(|_| ()),
                            None => transport::delete_page_builder_scenario_baseline(
                                token.clone(),
                                tenant.clone(),
                                page_id.clone(),
                                expected_baseline_hash,
                            )
                            .await
                            .map(|_| ()),
                        };
                        match write_result {
                            Ok(()) => match transport::fetch_page_builder_scenario_release_status(
                                token,
                                tenant,
                                page_id,
                            )
                            .await
                            {
                                Ok(status) => {
                                    server_status.set(status);
                                    persistence_error.set(None);
                                }
                                Err(error) => persistence_error.set(Some(format!(
                                    "Baseline was written but server status could not be verified: {error}"
                                ))),
                            },
                            Err(error) => persistence_error.set(Some(error.to_string())),
                        }
                    });
                },
            );

            let mut host = PageBuilderAdminHostContext::new(controller)
                .with_facade(facade)
                .with_runtime_context(generated_context)
                .with_runtime_scenarios(scenarios)
                .on_runtime_scenario_baseline(on_baseline);
            if let Some(baseline) = baseline {
                host = host.with_runtime_scenario_baseline(baseline);
            }
            provide_context(host);
            view! {
                <div class="space-y-2">
                    <ServerReleaseStatus status=server_status />
                    <PageBuilderAdmin />
                    {move || persistence_error.get().map(|error| view! {
                        <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">
                            {format!("Failed to persist scenario baseline: {error}")}
                        </div>
                    })}
                </div>
            }
            .into_any()
        }
        Err(error) => view! {
            <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">
                {error.to_string()}
            </div>
        }
        .into_any(),
    }
}

#[component]
fn ServerReleaseStatus(status: RwSignal<PageBuilderScenarioReleaseStatus>) -> impl IntoView {
    let class_status = status;
    let text_status = status;
    view! {
        <div
            class=move || {
                let status = class_status.get();
                if !status.allowed || status.status == "broken" || status.status == "baseline_invalid" {
                    "rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive"
                } else if status.status == "requires_review" {
                    "rounded-xl border border-amber-300/50 bg-amber-50 px-4 py-3 text-sm text-amber-900"
                } else {
                    "rounded-xl border border-border bg-muted/40 px-4 py-3 text-sm text-muted-foreground"
                }
            }
            role="status"
        >
            {move || {
                let status = text_status.get();
                format!(
                    "Server release gate: {} · allowed={} · {} visual · {} breaking{}",
                    status.status,
                    status.allowed,
                    status.visual_changes,
                    status.breaking_changes,
                    status
                        .baseline_hash
                        .as_deref()
                        .map(|hash| format!(" · baseline {hash}"))
                        .unwrap_or_default(),
                )
            }}
        </div>
    }
}
