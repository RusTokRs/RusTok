use crate::core::session::{EditorSessionAction, EditorSessionState};
use crate::i18n::t;
use crate::model::AdminCanvasController;
use crate::transport::PageBuilderAdminFacade;
use dioxus::prelude::*;
use fly::{RuntimeContextScenario, RuntimePublishGatePolicy, TraitSchemaRegistry};
use fly_dioxus::FlyFullEditor;
use fly_ui::{CapabilityState, ContributionAssemblyResult, EditorCapabilityEvaluation, UiIntent};
use rustok_ui_core::UiRouteContext;
use serde_json::Value;
use std::sync::Arc;

/// Host context for Dioxus Page Builder admin.
#[derive(Clone)]
pub struct DioxusPageBuilderAdminHostContext {
    pub controller: AdminCanvasController,
    pub facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    pub trait_schemas: Option<Arc<TraitSchemaRegistry>>,
    pub contribution_assembly: Option<Arc<ContributionAssemblyResult>>,
    pub editor_capabilities: Option<CapabilityState>,
    pub editor_capability_evaluation: Option<Arc<EditorCapabilityEvaluation>>,
    pub runtime_context: Option<Value>,
    pub runtime_scenarios: Option<Arc<Vec<RuntimeContextScenario>>>,
    pub runtime_publish_gate_policy: Option<Arc<RuntimePublishGatePolicy>>,
}

impl DioxusPageBuilderAdminHostContext {
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
        }
    }
}

impl PartialEq for DioxusPageBuilderAdminHostContext {
    fn eq(&self, other: &Self) -> bool {
        self.controller.page_id() == other.controller.page_id()
            && self.controller.revision_id() == other.controller.revision_id()
            && self.runtime_context == other.runtime_context
            && self.editor_capabilities == other.editor_capabilities
    }
}

/// Dioxus PageBuilderAdmin component.
#[component]
pub fn PageBuilderAdmin() -> Element {
    let host_context = use_context::<Option<DioxusPageBuilderAdminHostContext>>();
    let route_context = use_context::<Option<UiRouteContext>>().unwrap_or_default();
    let locale = route_context.locale;

    match host_context {
        Some(context) => rsx! {
            PageBuilderAdminWithController {
                controller: context.controller,
                facade: context.facade,
                trait_schemas: context.trait_schemas,
                contribution_assembly: context.contribution_assembly,
                editor_capabilities: context.editor_capabilities,
                editor_capability_evaluation: context.editor_capability_evaluation,
                runtime_context: context.runtime_context,
                runtime_scenarios: context.runtime_scenarios,
                runtime_publish_gate_policy: context.runtime_publish_gate_policy,
            }
        },
        None => {
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
            rsx! {
                section {
                    class: "rustok-page-builder-admin__unbound",
                    role: "status",
                    h2 { "{unbound_title}" }
                    p { "{unbound_body}" }
                }
            }
        }
    }
}

/// Props for Dioxus `PageBuilderAdminWithController`.
#[derive(Props, Clone)]
pub struct PageBuilderAdminWithControllerProps {
    pub controller: AdminCanvasController,
    #[props(default)]
    pub facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    #[props(default)]
    pub trait_schemas: Option<Arc<TraitSchemaRegistry>>,
    #[props(default)]
    pub contribution_assembly: Option<Arc<ContributionAssemblyResult>>,
    #[props(default)]
    pub editor_capabilities: Option<CapabilityState>,
    #[props(default)]
    pub editor_capability_evaluation: Option<Arc<EditorCapabilityEvaluation>>,
    #[props(default)]
    pub runtime_context: Option<Value>,
    #[props(default)]
    pub runtime_scenarios: Option<Arc<Vec<RuntimeContextScenario>>>,
    #[props(default)]
    pub runtime_publish_gate_policy: Option<Arc<RuntimePublishGatePolicy>>,
}

impl PartialEq for PageBuilderAdminWithControllerProps {
    fn eq(&self, other: &Self) -> bool {
        self.controller.page_id() == other.controller.page_id()
            && self.controller.revision_id() == other.controller.revision_id()
            && self.runtime_context == other.runtime_context
            && self.editor_capabilities == other.editor_capabilities
    }
}

/// Dioxus PageBuilderAdminWithController component.
#[component]
pub fn PageBuilderAdminWithController(props: PageBuilderAdminWithControllerProps) -> Element {
    let mut session = use_signal(|| {
        let mut s = EditorSessionState::new(
            props.controller.clone(),
            "Page Builder facade is missing",
            "Page saved successfully",
        );
        if let Some(schemas) = props.trait_schemas {
            s = s.with_trait_schemas(schemas);
        }
        if let Some(evaluation) = props.editor_capability_evaluation {
            s = s.with_editor_capability_evaluation(evaluation);
        }
        if let Some(ctx) = props.runtime_context {
            s = s.with_runtime_context(ctx);
        }
        if let Some(scenarios) = props.runtime_scenarios {
            s = s.with_runtime_scenarios(scenarios);
        }
        if let Some(policy) = props.runtime_publish_gate_policy {
            s = s.with_runtime_publish_gate_policy(policy);
        }
        s
    });

    let current = session.read();
    let page_id = current.controller.page_id().to_string();
    let last_error = current.last_error.clone();
    let last_announcement = current.last_announcement.clone();
    let can_undo = current.controller.can_undo();
    let can_redo = current.controller.can_redo();

    rsx! {
        FlyFullEditor {
            header {
                class: "rustok-page-builder-admin__toolbar flex items-center justify-between p-3 border-b",
                div {
                    class: "flex items-center gap-2",
                    span { class: "font-semibold", "Page: {page_id}" }
                    if let Some(announcement) = last_announcement {
                        span { class: "text-sm text-muted-foreground", "{announcement}" }
                    }
                }
                div {
                    class: "flex items-center gap-2",
                    button {
                        class: "px-3 py-1.5 text-sm rounded bg-muted hover:bg-muted/80 disabled:opacity-50",
                        disabled: !can_undo,
                        onclick: move |_| {
                            session.write().dispatch(EditorSessionAction::DispatchIntent(UiIntent::Undo));
                        },
                        "Undo"
                    }
                    button {
                        class: "px-3 py-1.5 text-sm rounded bg-muted hover:bg-muted/80 disabled:opacity-50",
                        disabled: !can_redo,
                        onclick: move |_| {
                            session.write().dispatch(EditorSessionAction::DispatchIntent(UiIntent::Redo));
                        },
                        "Redo"
                    }
                    button {
                        class: "px-3 py-1.5 text-sm rounded bg-primary text-primary-foreground hover:bg-primary/90",
                        onclick: move |_| {
                            session.write().dispatch(EditorSessionAction::DispatchIntent(UiIntent::RequestSave));
                        },
                        "Save"
                    }
                }
            }

            if let Some(err) = last_error {
                div {
                    class: "p-3 bg-destructive/10 text-destructive text-sm border-b border-destructive/20",
                    role: "alert",
                    "{err}"
                }
            }

            main {
                class: "rustok-page-builder-admin__canvas flex-1 p-4",
                div {
                    class: "canvas-viewport",
                    "Page Builder Canvas ({page_id})"
                }
            }
        }
    }
}
