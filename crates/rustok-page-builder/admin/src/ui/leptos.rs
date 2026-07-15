use crate::editor::{AdminCanvas, AdminShell};
use crate::i18n::t;
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

/// Host-provided composition context for a concrete consumer document.
///
/// Generated module composition mounts [`PageBuilderAdmin`] without props. Consumer routes such as
/// Pages may provide this context to activate a concrete document, persistence facade,
/// provider-contributed authoring schemas, preview-only runtime data, named preview scenarios,
/// runtime publish policy, and a separately persisted scenario release baseline.
#[derive(Clone)]
pub struct PageBuilderAdminHostContext {
    pub controller: AdminCanvasController,
    pub facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    pub trait_schemas: Option<Arc<TraitSchemaRegistry>>,
    pub runtime_context: Option<Value>,
    pub runtime_scenarios: Option<Arc<Vec<RuntimeContextScenario>>>,
    pub runtime_publish_gate_policy: Option<Arc<RuntimePublishGatePolicy>>,
    pub runtime_scenario_baseline: Option<RuntimeScenarioReleaseBaseline>,
    pub on_runtime_scenario_baseline: Option<Callback<PageBuilderScenarioBaselineChange>>,
}

impl PageBuilderAdminHostContext {
    pub fn new(controller: AdminCanvasController) -> Self {
        Self {
            controller,
            facade: None,
            trait_schemas: None,
            runtime_context: None,
            runtime_scenarios: None,
            runtime_publish_gate_policy: None,
            runtime_scenario_baseline: None,
            on_runtime_scenario_baseline: None,
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
                runtime_context=context.runtime_context
                runtime_scenarios=context.runtime_scenarios
                runtime_publish_gate_policy=context.runtime_publish_gate_policy
                runtime_scenario_baseline=context.runtime_scenario_baseline
                on_runtime_scenario_baseline=context.on_runtime_scenario_baseline
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
    controller: AdminCanvasController,
    facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    trait_schemas: Option<Arc<TraitSchemaRegistry>>,
    runtime_context: Option<Value>,
    runtime_scenarios: Option<Arc<Vec<RuntimeContextScenario>>>,
    runtime_publish_gate_policy: Option<Arc<RuntimePublishGatePolicy>>,
    runtime_scenario_baseline: Option<RuntimeScenarioReleaseBaseline>,
    on_runtime_scenario_baseline: Option<Callback<PageBuilderScenarioBaselineChange>>,
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

    view! {
        <AdminShell title subtitle>
            <AdminCanvas
                controller
                facade
                trait_schemas
                runtime_context
                runtime_scenarios
                runtime_publish_gate_policy
                runtime_scenario_baseline
                on_runtime_scenario_baseline
                on_request
            />
        </AdminShell>
    }
}
