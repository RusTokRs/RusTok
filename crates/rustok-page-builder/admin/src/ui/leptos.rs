use crate::editor::{AdminCanvas, AdminShell};
use crate::i18n::t;
use crate::{AdminCanvasController, PageBuilderAdminFacade, PageBuilderContributionHostContext};
use fly::{
    RuntimeContextScenario, RuntimePublishGatePolicy, RuntimeScenarioReleaseBaseline,
    TraitSchemaRegistry,
};
use fly_ui::{
    CapabilityState, ContributionAssemblyResult, EditorCapabilityEvaluation, EditorCapabilityPolicy,
};
use leptos::prelude::*;
use rustok_page_builder::dto::PageBuilderCapabilityRequest;
use rustok_page_builder::runtime_scenario_release::PageBuilderScenarioBaselineChange;
use rustok_ui_core::UiRouteContext;
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::Arc;

/// Host-provided composition context for a concrete consumer document.
///
/// Generated module composition mounts [`PageBuilderAdmin`] without props. Consumer routes such as
/// Pages may provide this context to activate a concrete document, persistence facade,
/// provider-contributed authoring schemas and registries, an evaluated tenant/RBAC/provider-health
/// capability profile, preview-only runtime data, named preview scenarios, runtime publish policy,
/// a separately persisted scenario release baseline, and an optional classic SSR intent endpoint
/// consumed by the standalone `fly-browser` adapter.
#[derive(Clone)]
pub struct PageBuilderAdminHostContext {
    pub controller: AdminCanvasController,
    pub facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    pub trait_schemas: Option<Arc<TraitSchemaRegistry>>,
    pub contribution_assembly: Option<Arc<ContributionAssemblyResult>>,
    pub editor_capabilities: Option<CapabilityState>,
    pub editor_capability_evaluation: Option<Arc<EditorCapabilityEvaluation>>,
    pub runtime_context: Option<Value>,
    pub runtime_scenarios: Option<Arc<Vec<RuntimeContextScenario>>>,
    pub runtime_publish_gate_policy: Option<Arc<RuntimePublishGatePolicy>>,
    pub runtime_scenario_baseline: Option<RuntimeScenarioReleaseBaseline>,
    pub on_runtime_scenario_baseline: Option<Callback<PageBuilderScenarioBaselineChange>>,
    pub browser_intent_endpoint: Option<String>,
    pub browser_csrf_token: Option<String>,
}

impl PageBuilderAdminHostContext {
    pub fn new(controller: AdminCanvasController) -> Self {
        Self {
            controller,
            facade: None,
            trait_schemas: None,
            contribution_assembly: None,
            editor_capabilities: None,
            editor_capability_evaluation: None,
            runtime_context: None,
            runtime_scenarios: None,
            runtime_publish_gate_policy: None,
            runtime_scenario_baseline: None,
            on_runtime_scenario_baseline: None,
            browser_intent_endpoint: None,
            browser_csrf_token: None,
        }
    }

    pub fn with_facade(mut self, facade: Arc<dyn PageBuilderAdminFacade>) -> Self {
        self.facade = Some(facade);
        self
    }

    pub fn with_trait_schemas(mut self, trait_schemas: Arc<TraitSchemaRegistry>) -> Self {
        self.trait_schemas = Some(trait_schemas);
        self
    }

    pub fn with_contribution_assembly(
        mut self,
        contribution_assembly: Arc<ContributionAssemblyResult>,
    ) -> Self {
        self.contribution_assembly = Some(contribution_assembly);
        self
    }

    pub fn with_editor_capabilities(mut self, capabilities: CapabilityState) -> Self {
        self.editor_capabilities = Some(capabilities.normalized());
        self.editor_capability_evaluation = None;
        self
    }

    pub fn with_editor_capability_policy(mut self, policy: EditorCapabilityPolicy) -> Self {
        let evaluation = Arc::new(policy.evaluate_detailed());
        self.editor_capabilities = Some(evaluation.effective);
        self.editor_capability_evaluation = Some(evaluation);
        self
    }

    pub fn with_editor_capability_evaluation(
        mut self,
        evaluation: Arc<EditorCapabilityEvaluation>,
    ) -> Self {
        self.editor_capabilities = Some(evaluation.effective);
        self.editor_capability_evaluation = Some(evaluation);
        self
    }

    pub fn with_runtime_context(mut self, runtime_context: Value) -> Self {
        self.runtime_context = Some(runtime_context);
        self
    }

    pub fn with_runtime_scenarios(
        mut self,
        runtime_scenarios: Arc<Vec<RuntimeContextScenario>>,
    ) -> Self {
        self.runtime_scenarios = Some(runtime_scenarios);
        self
    }

    pub fn with_runtime_publish_gate_policy(
        mut self,
        policy: Arc<RuntimePublishGatePolicy>,
    ) -> Self {
        self.runtime_publish_gate_policy = Some(policy);
        self
    }

    pub fn with_runtime_scenario_baseline(
        mut self,
        baseline: RuntimeScenarioReleaseBaseline,
    ) -> Self {
        self.runtime_scenario_baseline = Some(baseline);
        self
    }

    pub fn on_runtime_scenario_baseline(
        mut self,
        callback: Callback<PageBuilderScenarioBaselineChange>,
    ) -> Self {
        self.on_runtime_scenario_baseline = Some(callback);
        self
    }

    pub fn with_browser_intent_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        self.browser_intent_endpoint = (!endpoint.trim().is_empty()).then_some(endpoint);
        self
    }

    pub fn with_browser_csrf_token(mut self, token: impl Into<String>) -> Self {
        let token = token.into();
        self.browser_csrf_token = (!token.trim().is_empty()).then_some(token);
        self
    }
}

/// Generated host entrypoint. It intentionally accepts no props.
///
/// Without a consumer-owned document context the control-plane route remains useful and explicit:
/// it explains that document lifecycle belongs to Pages/Blog/Forum rather than fabricating an
/// unpersisted page inside the generic Page Builder module.
#[component]
pub fn PageBuilderAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;

    match use_context::<PageBuilderAdminHostContext>() {
        Some(context) => view! {
            <PageBuilderAdminWithController
                controller=context.controller
                facade=context.facade
                trait_schemas=context.trait_schemas
                contribution_assembly=context.contribution_assembly
                editor_capabilities=context.editor_capabilities
                editor_capability_evaluation=context.editor_capability_evaluation
                runtime_context=context.runtime_context
                runtime_scenarios=context.runtime_scenarios
                runtime_publish_gate_policy=context.runtime_publish_gate_policy
                runtime_scenario_baseline=context.runtime_scenario_baseline
                on_runtime_scenario_baseline=context.on_runtime_scenario_baseline
                browser_intent_endpoint=context.browser_intent_endpoint
                browser_csrf_token=context.browser_csrf_token
                on_request=None
            />
        }
        .into_any(),
        None => {
            let title = t(locale.as_deref(), "page_builder.title", "Page Builder");
            let subtitle = t(
                locale.as_deref(),
                "page_builder.subtitle",
                "Fly runtime, compatibility, and provider control surface.",
            );
            let unbound_title = t(
                locale.as_deref(),
                "page_builder.unbound.title",
                "No consumer document selected",
            );
            let unbound_body = t(
                locale.as_deref(),
                "page_builder.unbound.body",
                "Open a consumer-owned document to start full visual authoring. Page Builder does not own document persistence.",
            );

            view! {
                <AdminShell title subtitle>
                    <section class="rustok-page-builder-admin__unbound" role="status">
                        <h2>{unbound_title}</h2>
                        <p>{unbound_body}</p>
                    </section>
                </AdminShell>
            }
            .into_any()
        }
    }
}

#[component]
pub fn PageBuilderAdminWithController(
    mut controller: AdminCanvasController,
    facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    trait_schemas: Option<Arc<TraitSchemaRegistry>>,
    #[prop(optional_no_strip)] mut contribution_assembly: Option<Arc<ContributionAssemblyResult>>,
    #[prop(optional_no_strip)] editor_capabilities: Option<CapabilityState>,
    #[prop(optional_no_strip)] editor_capability_evaluation: Option<
        Arc<EditorCapabilityEvaluation>,
    >,
    runtime_context: Option<Value>,
    runtime_scenarios: Option<Arc<Vec<RuntimeContextScenario>>>,
    runtime_publish_gate_policy: Option<Arc<RuntimePublishGatePolicy>>,
    runtime_scenario_baseline: Option<RuntimeScenarioReleaseBaseline>,
    on_runtime_scenario_baseline: Option<Callback<PageBuilderScenarioBaselineChange>>,
    #[prop(optional_no_strip)] browser_intent_endpoint: Option<String>,
    #[prop(optional_no_strip)] browser_csrf_token: Option<String>,
    on_request: Option<Callback<PageBuilderCapabilityRequest>>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title_prefix = t(locale.as_deref(), "page_builder.title", "Page Builder");
    let title = format!("{title_prefix}: {}", controller.page_id());
    let subtitle = t(
        locale.as_deref(),
        "page_builder.editorSubtitle",
        "Full Fly authoring surface. Persistence remains owned by the consumer module facade.",
    );

    if let Some(extension_host) = use_context::<PageBuilderContributionHostContext>() {
        if !extension_host.is_empty() {
            if let Err(error) = controller.install_contribution_registries(|registries| {
                extension_host.install_registries(registries)
            }) {
                return view! {
                    <AdminShell title=title.clone() subtitle=subtitle.clone()>
                        <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">
                            {format!("Page Builder contribution registry installation failed: {error}")}
                        </div>
                    </AdminShell>
                }
                .into_any();
            }

            let host_effective = editor_capability_evaluation
                .as_ref()
                .map(|evaluation| evaluation.effective)
                .or(editor_capabilities)
                .unwrap_or_else(CapabilityState::full);
            let provider_status = facade.as_ref().and_then(|facade| facade.provider_status());
            let effective = provider_status
                .as_ref()
                .map(|status| status.limit_capabilities(host_effective))
                .unwrap_or(host_effective);
            let preview_enabled = provider_status
                .as_ref()
                .map(|status| status.preview_enabled())
                .unwrap_or(true);
            contribution_assembly = Some(extension_host.merge_admin_assembly(
                contribution_assembly,
                contribution_capabilities(effective, preview_enabled),
            ));
        }
    }

    view! {
        <AdminShell title subtitle>
            <AdminCanvas
                controller
                facade
                trait_schemas
                contribution_assembly
                editor_capabilities
                editor_capability_evaluation
                runtime_context
                runtime_scenarios
                runtime_publish_gate_policy
                runtime_scenario_baseline
                on_runtime_scenario_baseline
                on_request
                browser_intent_endpoint
                browser_csrf_token
            />
        </AdminShell>
    }
    .into_any()
}

fn contribution_capabilities(
    capabilities: CapabilityState,
    preview_enabled: bool,
) -> BTreeSet<String> {
    let capabilities = capabilities.normalized();
    let mut granted = BTreeSet::new();
    if capabilities.edit {
        granted.insert("tree".to_string());
    }
    if capabilities.properties {
        granted.insert("properties".to_string());
    }
    if capabilities.publish {
        granted.insert("publish".to_string());
    }
    if preview_enabled {
        granted.insert("preview".to_string());
    }
    granted
}
