use crate::editor::{
    AdminEditorRuntime, AuthoringToolbar, BindingPanel, ContextCompatibilityPanel,
    ContextContractToolsPanel, ContextDependencyPanel, ContextSchemaPanel, DynamicRuntimePanel,
    IsolatedAuthoringCanvas, PageManagerPanel, PaletteLayersPanel, PropertiesAssetsPanel,
    ResponsiveStylePanel, RuntimePublishGatePanel, RuntimeScenarioMatrixPanel,
    RuntimeScenarioPanel, RuntimeScenarioRegressionPanel, SsrInspectorPanel, SsrLocalePanel,
    TraitPanel,
};
use crate::i18n::t;
use crate::ui::browser_adapter::PageBuilderBrowserAdapter;
use crate::{AdminCanvasController, PageBuilderAdminFacade};
use fly::{
    RuntimeContextScenario, RuntimePublishGatePolicy, RuntimeScenarioReleaseBaseline,
    TraitSchemaRegistry,
};
use leptos::prelude::*;
use rustok_page_builder::dto::PageBuilderCapabilityRequest;
use rustok_page_builder::runtime_scenario_release::PageBuilderScenarioBaselineChange;
use rustok_ui_core::UiRouteContext;
use serde_json::Value;
use std::sync::Arc;

#[component]
pub fn AdminCanvas(
    controller: AdminCanvasController,
    facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    trait_schemas: Option<Arc<TraitSchemaRegistry>>,
    runtime_context: Option<Value>,
    runtime_scenarios: Option<Arc<Vec<RuntimeContextScenario>>>,
    runtime_publish_gate_policy: Option<Arc<RuntimePublishGatePolicy>>,
    runtime_scenario_baseline: Option<RuntimeScenarioReleaseBaseline>,
    on_runtime_scenario_baseline: Option<Callback<PageBuilderScenarioBaselineChange>>,
    on_request: Option<Callback<PageBuilderCapabilityRequest>>,
    #[prop(optional)] browser_intent_endpoint: Option<String>,
    #[prop(optional)] browser_csrf_token: Option<String>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let facade_missing = t(
        locale.as_deref(),
        "page_builder.facadeMissing",
        "Page Builder admin facade is not mounted for this canvas",
    );
    let save_succeeded = t(
        locale.as_deref(),
        "page_builder.status.saveSucceeded",
        "Project saved",
    );
    let runtime = AdminEditorRuntime::new(
        controller,
        facade,
        on_request,
        facade_missing,
        save_succeeded,
    );
    let runtime = match trait_schemas {
        Some(trait_schemas) => runtime.with_trait_schemas(trait_schemas),
        None => runtime,
    };
    let runtime = match runtime_context {
        Some(runtime_context) => runtime.with_runtime_context(runtime_context),
        None => runtime,
    };
    let runtime = match runtime_scenarios {
        Some(runtime_scenarios) => runtime.with_runtime_scenarios(runtime_scenarios),
        None => runtime,
    };
    let runtime = match runtime_publish_gate_policy {
        Some(policy) => runtime.with_runtime_publish_gate_policy(policy),
        None => runtime,
    };
    let browser_page_id = runtime
        .controller
        .with(|controller| controller.page_id().to_string());
    let browser_revision = runtime
        .controller
        .with(|controller| controller.revision_id().to_string());
    let browser_project_hash = runtime
        .controller
        .with(|controller| controller.editor().revision().project_hash.hex());
    let root_intent_endpoint = browser_intent_endpoint.clone();
    let root_csrf_token = browser_csrf_token.clone();
    let toolbar_runtime = runtime.clone();
    let page_runtime = runtime.clone();
    let palette_runtime = runtime.clone();
    let canvas_runtime = runtime.clone();
    let gate_runtime = runtime.clone();
    let scenario_runtime = runtime.clone();
    let scenario_matrix_runtime = runtime.clone();
    let scenario_regression_runtime = runtime.clone();
    let dynamic_runtime = runtime.clone();
    let context_runtime = runtime.clone();
    let contract_tools_runtime = runtime.clone();
    let compatibility_runtime = runtime.clone();
    let dependency_runtime = runtime.clone();
    let binding_runtime = runtime.clone();
    let trait_runtime = runtime.clone();
    let properties_runtime = runtime.clone();
    let responsive_runtime = runtime.clone();
    let ssr_locale_runtime = runtime.clone();
    let ssr_inspector_runtime = runtime.clone();
    let announcement_runtime = runtime.clone();
    let error_runtime = runtime;

    view! {
        <div
            class="rustok-page-builder-admin__workspace space-y-3"
            data-fly-browser-root="true"
            data-fly-runtime="ssr"
            data-fly-page-id=browser_page_id
            data-fly-revision=browser_revision
            data-fly-project-hash=browser_project_hash
            data-fly-intent-endpoint=root_intent_endpoint
            data-fly-csrf-token=root_csrf_token
        >
            <PageBuilderBrowserAdapter
                intent_endpoint=browser_intent_endpoint
                csrf_token=browser_csrf_token
            />
            <AuthoringToolbar runtime=toolbar_runtime />
            <div
                class="grid min-h-[680px] gap-3"
                style="grid-template-columns:minmax(240px,300px) minmax(420px,1fr) minmax(280px,360px)"
            >
                <div class="space-y-3 overflow-auto">
                    <PageManagerPanel runtime=page_runtime />
                    <PaletteLayersPanel runtime=palette_runtime />
                </div>
                <IsolatedAuthoringCanvas runtime=canvas_runtime />
                <div class="space-y-3 overflow-auto">
                    <SsrLocalePanel runtime=ssr_locale_runtime />
                    <SsrInspectorPanel runtime=ssr_inspector_runtime />
                    <RuntimePublishGatePanel runtime=gate_runtime />
                    <RuntimeScenarioPanel runtime=scenario_runtime />
                    <RuntimeScenarioMatrixPanel runtime=scenario_matrix_runtime />
                    <RuntimeScenarioRegressionPanel
                        runtime=scenario_regression_runtime
                        initial_baseline=runtime_scenario_baseline
                        on_baseline_change=on_runtime_scenario_baseline
                    />
                    <DynamicRuntimePanel runtime=dynamic_runtime />
                    <ContextSchemaPanel runtime=context_runtime />
                    <ContextContractToolsPanel runtime=contract_tools_runtime />
                    <ContextCompatibilityPanel runtime=compatibility_runtime />
                    <ContextDependencyPanel runtime=dependency_runtime />
                    <BindingPanel runtime=binding_runtime />
                    <TraitPanel runtime=trait_runtime />
                    <PropertiesAssetsPanel runtime=properties_runtime />
                    <ResponsiveStylePanel runtime=responsive_runtime />
                </div>
            </div>
            <div class="space-y-2" aria-live="polite">
                {move || announcement_runtime.last_announcement.get().map(|message| view! {
                    <p class="rounded bg-muted px-3 py-2 text-sm">{message}</p>
                })}
                {move || error_runtime.last_error.get().map(|message| view! {
                    <p class="rounded bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">{message}</p>
                })}
            </div>
        </div>
    }
}
