use crate::editor::{AdminCanvas, AdminShell};
use crate::i18n::t;
use crate::{AdminCanvasController, PageBuilderAdminFacade};
use fly::{RuntimeContextScenario, TraitSchemaRegistry};
use leptos::prelude::*;
use rustok_page_builder::dto::PageBuilderCapabilityRequest;
use rustok_ui_core::UiRouteContext;
use serde_json::Value;
use std::sync::Arc;

/// Host-provided composition context for a concrete consumer document.
///
/// Generated module composition mounts [`PageBuilderAdmin`] without props. Consumer routes such as
/// Pages may provide this context to activate a concrete document, persistence facade,
/// provider-contributed authoring schemas, preview-only runtime data, and named preview scenarios.
#[derive(Clone)]
pub struct PageBuilderAdminHostContext {
    pub controller: AdminCanvasController,
    pub facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    pub trait_schemas: Option<Arc<TraitSchemaRegistry>>,
    pub runtime_context: Option<Value>,
    pub runtime_scenarios: Option<Arc<Vec<RuntimeContextScenario>>>,
}

impl PageBuilderAdminHostContext {
    pub fn new(controller: AdminCanvasController) -> Self {
        Self {
            controller,
            facade: None,
            trait_schemas: None,
            runtime_context: None,
            runtime_scenarios: None,
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
                <AdminShell title subtitle=Some(subtitle)>
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
    #[prop(optional)] facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    #[prop(optional)] trait_schemas: Option<Arc<TraitSchemaRegistry>>,
    #[prop(optional)] runtime_context: Option<Value>,
    #[prop(optional)] runtime_scenarios: Option<Arc<Vec<RuntimeContextScenario>>>,
    #[prop(optional)] on_request: Option<Callback<PageBuilderCapabilityRequest>>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title_prefix = t(locale.as_deref(), "page_builder.title", "Page Builder");
    let title = format!("{title_prefix}: {}", controller.page_id());
    let subtitle = Some(t(
        locale.as_deref(),
        "page_builder.editorSubtitle",
        "Full Fly authoring surface. Persistence remains owned by the consumer module facade.",
    ));

    view! {
        <AdminShell title subtitle>
            <AdminCanvas
                controller
                facade
                trait_schemas
                runtime_context
                runtime_scenarios
                on_request
            />
        </AdminShell>
    }
}
