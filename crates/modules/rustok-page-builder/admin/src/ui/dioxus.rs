use crate::core::session::{EditorSessionAction, EditorSessionEffect, EditorSessionState};
use crate::i18n::t;
use crate::model::AdminCanvasController;
use crate::BrowserIntentEffect;
use crate::transport::PageBuilderAdminFacade;
use dioxus::prelude::*;
use fly::TraitSchemaRegistry;
use fly_browser::BrowserIntentEnvelope;
use fly_dioxus::{FlyEditorWorkspace, FlyFullEditor};
use fly_ui::{
    CapabilityState, ContributionAssemblyResult, EditorCapabilityEvaluation, EditorMessages,
    UiIntent,
};
use rustok_page_builder::{RuntimeContextScenario, RuntimePublishGatePolicy};
use rustok_page_builder::dto::{
    BuilderCapabilityKind, PageBuilderCapabilityRequest, PageBuilderCapabilityResponse,
};
use rustok_ui_core::{UiCspNonce, UiRouteContext};
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
    pub adapter_workspace: bool,
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
            adapter_workspace: false,
        }
    }

    /// Mounts the shared adapter workspace (`fly-dioxus`) instead of the canvas
    /// placeholder. Off by default; hosts opt in while the admin migration
    /// (P0-1) proceeds surface by surface.
    pub fn with_adapter_workspace(mut self, enabled: bool) -> Self {
        self.adapter_workspace = enabled;
        self
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

/// Deliver the command effects the shared editor reducer emits. Previously the
/// Dioxus toolbar and workspace discarded Publish/Preview effects, so Save
/// visibly did nothing even when a consumer supplied a working facade.
fn dispatch_facade_effects(
    mut session: Signal<EditorSessionState>,
    facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    effects: Vec<EditorSessionEffect>,
) {
    for effect in effects {
        match effect {
            EditorSessionEffect::Announce(message) => {
                session.write().announce(message);
            }
            EditorSessionEffect::ExecuteRequest {
                request,
                expected_hash,
            } => {
                let Some(facade) = facade.clone() else {
                    session.write().fail("Page Builder facade is missing");
                    continue;
                };
                let capability = request.capability();
                let save_attempt_id = if capability == BuilderCapabilityKind::Publish {
                    if expected_hash.is_none() {
                        session.write().fail("Publish request has no submitted project hash");
                        continue;
                    }
                    match session.write().mark_save_started_with_id() {
                        Ok(id) => Some(id),
                        Err(error) => {
                            session.write().fail(error);
                            continue;
                        }
                    }
                } else {
                    None
                };
                if let PageBuilderCapabilityRequest::Preview(input) = &request {
                    let mut state = session.write();
                    if state.preview_in_progress && state.preview_request.is_none() {
                        state.fail("Preview is already in progress");
                        continue;
                    }
                    if !state.preview_in_progress {
                        // Browser-origin previews supply runtime context in the
                        // request instead of calling prepare_server_preview_request.
                        let context = input.runtime.context.clone();
                        let scenario = input.runtime.scenario_id.clone();
                        state.runtime_context = context.clone();
                        state.runtime_context_configured = true;
                        state.active_runtime_scenario = scenario.clone();
                        state.preview_request = Some((
                            state.controller.editor().revision().project_hash,
                            state.controller.active_page_index(),
                            context,
                            scenario,
                        ));
                        state.preview_in_progress = true;
                    }
                }
                spawn(async move {
                    let outcome = facade.execute(request).await;
                    let mut state = session.write();
                    match outcome {
                        Ok(PageBuilderCapabilityResponse::Publish(response))
                            if capability == BuilderCapabilityKind::Publish =>
                        {
                            let Some(attempt_id) = save_attempt_id else {
                                return;
                            };
                            if state.controller.active_save_attempt_id() != Some(attempt_id) {
                                return;
                            }
                            let _ = state.handle_publish_response_for_attempt(
                                &response.page_id,
                                &response.revision_id,
                                expected_hash,
                                attempt_id,
                            );
                        }
                        Ok(PageBuilderCapabilityResponse::Preview(response))
                            if capability == BuilderCapabilityKind::Preview =>
                        {
                            let _ = state.handle_preview_response(&response);
                        }
                        Ok(other) => {
                            let error = format!(
                                "Page Builder facade returned `{}` for a `{capability}` request",
                                other.capability()
                            );
                            if let Some(id) = save_attempt_id {
                                state.handle_publish_error_for_attempt(id, error);
                            } else {
                                state.preview_in_progress = false;
                                state.preview_request = None;
                                state.fail(error);
                            }
                        }
                        Err(error) => {
                            if let Some(id) = save_attempt_id {
                                state.handle_publish_error_for_attempt(id, error.to_string());
                            } else {
                                state.preview_in_progress = false;
                                state.preview_request = None;
                                state.fail(error.to_string());
                            }
                        }
                    }
                });
            }
        }
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
                adapter_workspace: context.adapter_workspace,
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
    #[props(default)]
    pub adapter_workspace: bool,
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
    let route_context = use_context::<Option<UiRouteContext>>().unwrap_or_default();
    let locale = route_context.locale;
    // The workspace renders the project stylesheet inline; without the request nonce the host's
    // policy drops it and the canvas renders unstyled.
    let workspace_csp_nonce = use_context::<Option<UiCspNonce>>()
        .flatten()
        .map(|nonce| nonce.as_str().to_string());
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
    let workspace_document = current.controller.editor().document().clone();
    let workspace_page_id = page_id.clone();
    let last_error = current.last_error.clone();
    let last_announcement = current.last_announcement.clone();
    let can_undo = current.controller.can_undo();
    let can_redo = current.controller.can_redo();

    let route_locale = locale.clone();
    let workspace_messages: Option<EditorMessages> = props
        .adapter_workspace
        .then(|| crate::editor::adapter_editor_messages(route_locale.as_deref()));
    let workspace_assembly = props
        .contribution_assembly
        .as_ref()
        .map(|assembly| assembly.as_ref().clone());
    let show_adapter_workspace = props.adapter_workspace;
    let workspace_facade = props.facade.clone();
    let toolbar_facade = props.facade.clone();
    let on_workspace_intent = EventHandler::new(move |envelope: BrowserIntentEnvelope| {
        let effects = {
            let mut state = session.write();
            // The trusted Dioxus callback owns the current host snapshot;
            // typed Fly constructors do not know its revision.
            let mut envelope = envelope;
            envelope.revision = Some(state.controller.revision_id().to_string());
            envelope.project_hash = Some(state.controller.editor().revision().project_hash.hex());
            match crate::dispatch_browser_intent(&mut state.controller, envelope) {
                Ok(result) => {
                    state.last_error = None;
                    let current_hash = state.controller.editor().revision().project_hash;
                    result.effects.into_iter().map(|effect| match effect {
                        BrowserIntentEffect::Announce { message } =>
                            EditorSessionEffect::Announce(message),
                        BrowserIntentEffect::Request { request, expected_hash, .. } => {
                            // The request was produced synchronously by this
                            // controller; never substitute a mismatched hash.
                            let expected_hash = expected_hash
                                .filter(|hex| hex == &current_hash.hex())
                                .map(|_| current_hash);
                            EditorSessionEffect::ExecuteRequest { request, expected_hash }
                        }
                    }).collect::<Vec<_>>()
                }
                Err(error) => {
                    state.fail(error.to_string());
                    Vec::new()
                }
            }
        };
        dispatch_facade_effects(session, workspace_facade.clone(), effects);
    });

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
                            let effects = session.write().dispatch(
                                EditorSessionAction::DispatchIntent(UiIntent::RequestSave)
                            );
                            dispatch_facade_effects(session, toolbar_facade.clone(), effects);
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
                match show_adapter_workspace {
                    true => rsx! {
                        FlyEditorWorkspace {
                            document: workspace_document.clone(),
                            contribution_assembly: workspace_assembly.clone(),
                            instance_id: Some(workspace_page_id.clone()),
                            on_intent: Some(on_workspace_intent),
                            on_change: None,
                            messages: workspace_messages.clone(),
                            csp_nonce: workspace_csp_nonce.clone(),
                        }
                    },
                    false => rsx! {
                        div {
                            class: "canvas-viewport",
                            "Page Builder Canvas ({page_id})"
                        }
                    },
                }
            }
        }
    }
}
