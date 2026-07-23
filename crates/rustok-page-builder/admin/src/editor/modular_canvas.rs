use crate::editor::{
    AdminEditorRuntime, AuditPanel, AuthoringToolbar, BindingPanel, CapabilityPolicyPanel,
    ConsumerPropertiesPanel, ContextContractToolsPanel, ContextDependencyPanel, ContextSchemaPanel,
    DynamicRuntimePanel, IsolatedAuthoringCanvas, PageManagerPanel, PaletteLayersPanel,
    PropertiesAssetsPanel, PublishScenarioSelectorPanel, ResponsiveStylePanel,
    RuntimePublishGatePanel, RuntimeScenarioMatrixPanel, RuntimeScenarioPanel,
    RuntimeScenarioRegressionPanel, ServerPreviewPanel, SsrActionsFormsPanel, SsrAssetPanel,
    SsrInspectorPanel, SsrInternalPageLinkPanel, SsrLocaleCoveragePanel, SsrLocalePanel,
    SsrLocalePolicyPanel, SsrLocalizedMetadataPanel, SsrTranslationsPanel, TraitPanel,
};
use crate::i18n::t;
use crate::ui::browser_adapter::PageBuilderBrowserAdapter;
use crate::{
    AdminCanvasController, ConsumerPropertyEditorRuntime, PageBuilderAdminFacade,
};
use fly::{
    RuntimeContextScenario, RuntimePublishGatePolicy, RuntimeScenarioReleaseBaseline,
    TraitSchemaRegistry,
};
use fly_ui::{CapabilityState, ContributionAssemblyResult, EditorCapabilityEvaluation, UiIntent};
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
    #[prop(optional_no_strip)] contribution_assembly: Option<Arc<ContributionAssemblyResult>>,
    #[prop(optional_no_strip)] editor_capabilities: Option<CapabilityState>,
    #[prop(optional_no_strip)] editor_capability_evaluation: Option<
        Arc<EditorCapabilityEvaluation>,
    >,
    runtime_context: Option<Value>,
    runtime_scenarios: Option<Arc<Vec<RuntimeContextScenario>>>,
    runtime_publish_gate_policy: Option<Arc<RuntimePublishGatePolicy>>,
    runtime_scenario_baseline: Option<RuntimeScenarioReleaseBaseline>,
    on_runtime_scenario_baseline: Option<Callback<PageBuilderScenarioBaselineChange>>,
    on_request: Option<Callback<PageBuilderCapabilityRequest>>,
    #[prop(optional_no_strip)] browser_intent_endpoint: Option<String>,
    #[prop(optional_no_strip)] browser_csrf_token: Option<String>,
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
    let evaluated_capabilities = editor_capability_evaluation
        .as_ref()
        .map(|evaluation| evaluation.effective);
    let consumer_properties = facade
        .as_ref()
        .and_then(|facade| facade.consumer_properties())
        .or_else(|| use_context::<Arc<ConsumerPropertyEditorRuntime>>());
    let consumer_property_assembly = contribution_assembly.clone();
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
    let runtime = match editor_capability_evaluation {
        Some(evaluation) => runtime.with_editor_capability_evaluation(evaluation),
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
    if let Some(capabilities) = editor_capabilities.or(evaluated_capabilities) {
        runtime.dispatch(UiIntent::SetEditableCapabilities(capabilities));
    }

    let scenario_baseline = RwSignal::new(runtime_scenario_baseline);
    let host_baseline_callback = StoredValue::new(on_runtime_scenario_baseline);
    let baseline_signal = scenario_baseline;
    let on_baseline_change = Callback::new(move |change: PageBuilderScenarioBaselineChange| {
        baseline_signal.set(change.baseline.clone());
        if let Some(callback) = host_baseline_callback.get_value() {
            callback.run(change);
        }
    });

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
    let server_preview_runtime = runtime.clone();
    let page_runtime = runtime.clone();
    let palette_runtime = runtime.clone();
    let canvas_runtime = runtime.clone();
    let capability_runtime = runtime.clone();
    let audit_runtime = runtime.clone();
    let gate_runtime = runtime.clone();
    let scenario_runtime = runtime.clone();
    let scenario_matrix_runtime = runtime.clone();
    let publish_scenario_runtime = runtime.clone();
    let scenario_regression_runtime = runtime.clone();
    let dynamic_runtime = runtime.clone();
    let context_runtime = runtime.clone();
    let contract_tools_runtime = runtime.clone();
    let dependency_runtime = runtime.clone();
    let binding_runtime = runtime.clone();
    let trait_runtime = runtime.clone();
    let properties_runtime = runtime.clone();
    let responsive_runtime = runtime.clone();
    let ssr_locale_runtime = runtime.clone();
    let ssr_locale_policy_runtime = runtime.clone();
    let ssr_locale_coverage_runtime = runtime.clone();
    let ssr_translations_runtime = runtime.clone();
    let ssr_localized_metadata_runtime = runtime.clone();
    let ssr_internal_link_runtime = runtime.clone();
    let ssr_actions_runtime = runtime.clone();
    let ssr_assets_runtime = runtime.clone();
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
            <ServerPreviewPanel runtime=server_preview_runtime />
            <div class="grid min-h-[680px] grid-cols-[minmax(240px,300px)_minmax(420px,1fr)_minmax(280px,360px)] gap-3">
                <div class="space-y-3 overflow-auto">
                    <PageManagerPanel runtime=page_runtime />
                    <PaletteLayersPanel
                        runtime=palette_runtime
                        contribution_assembly
                    />
                </div>
                <IsolatedAuthoringCanvas runtime=canvas_runtime />
                <div class="space-y-3 overflow-auto">
                    <CapabilityPolicyPanel runtime=capability_runtime />
                    <ConsumerPropertiesPanel
                        runtime=consumer_properties
                        contribution_assembly=consumer_property_assembly
                    />
                    <SsrLocalePanel runtime=ssr_locale_runtime />
                    <SsrLocalePolicyPanel runtime=ssr_locale_policy_runtime />
                    <SsrLocaleCoveragePanel runtime=ssr_locale_coverage_runtime />
                    <SsrTranslationsPanel runtime=ssr_translations_runtime />
                    <SsrLocalizedMetadataPanel runtime=ssr_localized_metadata_runtime />
                    <SsrInternalPageLinkPanel runtime=ssr_internal_link_runtime />
                    <SsrActionsFormsPanel runtime=ssr_actions_runtime />
                    <SsrAssetPanel runtime=ssr_assets_runtime />
                    <SsrInspectorPanel runtime=ssr_inspector_runtime />
                    <AuditPanel runtime=audit_runtime />
                    <RuntimePublishGatePanel runtime=gate_runtime />
                    <RuntimeScenarioPanel runtime=scenario_runtime />
                    <RuntimeScenarioMatrixPanel runtime=scenario_matrix_runtime />
                    <PublishScenarioSelectorPanel
                        runtime=publish_scenario_runtime
                        baseline=scenario_baseline
                    />
                    <RuntimeScenarioRegressionPanel
                        runtime=scenario_regression_runtime
                        initial_baseline=scenario_baseline.get_untracked()
                        on_baseline_change=Some(on_baseline_change)
                    />
                    <DynamicRuntimePanel runtime=dynamic_runtime />
                    <ContextSchemaPanel runtime=context_runtime />
                    <ContextContractToolsPanel runtime=contract_tools_runtime />
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
