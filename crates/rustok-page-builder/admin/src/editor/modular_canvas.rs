use crate::editor::{
    AdminEditorRuntime, AuthoringToolbar, BindingPanel, ContextCompatibilityPanel,
    ContextContractToolsPanel, ContextDependencyPanel, ContextSchemaPanel, DynamicRuntimePanel,
    IsolatedAuthoringCanvas, PageManagerPanel, PaletteLayersPanel, PropertiesAssetsPanel,
    ResponsiveStylePanel, RuntimePublishGatePanel, RuntimeScenarioMatrixPanel,
    RuntimeScenarioPanel, TraitPanel,
};
use crate::i18n::t;
use crate::{AdminCanvasController, PageBuilderAdminFacade};
use fly::{RuntimeContextScenario, RuntimePublishGatePolicy, TraitSchemaRegistry};
use leptos::prelude::*;
use rustok_page_builder::dto::PageBuilderCapabilityRequest;
use rustok_ui_core::UiRouteContext;
use serde_json::Value;
use std::sync::Arc;

#[component]
pub fn AdminCanvas(
    controller: AdminCanvasController,
    #[prop(optional)] facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    #[prop(optional)] trait_schemas: Option<Arc<TraitSchemaRegistry>>,
    #[prop(optional)] runtime_context: Option<Value>,
    #[prop(optional)] runtime_scenarios: Option<Arc<Vec<RuntimeContextScenario>>>,
    #[prop(optional)] runtime_publish_gate_policy: Option<Arc<RuntimePublishGatePolicy>>,
    #[prop(optional)] on_request: Option<Callback<PageBuilderCapabilityRequest>>,
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
    let toolbar_runtime = runtime.clone();
    let page_runtime = runtime.clone();
    let palette_runtime = runtime.clone();
    let canvas_runtime = runtime.clone();
    let gate_runtime = runtime.clone();
    let scenario_runtime = runtime.clone();
    let scenario_matrix_runtime = runtime.clone();
    let dynamic_runtime = runtime.clone();
    let context_runtime = runtime.clone();
    let contract_tools_runtime = runtime.clone();
    let compatibility_runtime = runtime.clone();
    let dependency_runtime = runtime.clone();
    let binding_runtime = runtime.clone();
    let trait_runtime = runtime.clone();
    let properties_runtime = runtime.clone();
    let responsive_runtime = runtime.clone();
    let announcement_runtime = runtime.clone();
    let error_runtime = runtime;

    view! {
        <div class="rustok-page-builder-admin__workspace space-y-3">
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
                    <RuntimePublishGatePanel runtime=gate_runtime />
                    <RuntimeScenarioPanel runtime=scenario_runtime />
                    <RuntimeScenarioMatrixPanel runtime=scenario_matrix_runtime />
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
