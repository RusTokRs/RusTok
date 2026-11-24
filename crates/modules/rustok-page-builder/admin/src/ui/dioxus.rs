use crate::core::session::{EditorSessionAction, EditorSessionEffect, EditorSessionState};
#[cfg(feature = "dioxus-web")]
use super::dioxus_canvas::DioxusIsolatedCanvas;
#[cfg(feature = "dioxus-web")]
use crate::editor::render_canvas_srcdoc_with_context;
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

#[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
use fly_web::{BrowserPeerWriteChannel, BrowserPeerWriteConflict};

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
            adapter_workspace: true,
        }
    }

    /// Mounts the shared Fly adapter workspace. Enabled by default; hosts may
    /// explicitly opt out when they need the minimal placeholder shell.
    pub fn with_adapter_workspace(mut self, enabled: bool) -> Self {
        self.adapter_workspace = enabled;
        self
    }
}

fn same_optional_arc<T: ?Sized>(left: &Option<Arc<T>>, right: &Option<Arc<T>>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => Arc::ptr_eq(left, right),
        _ => false,
    }
}

impl PartialEq for DioxusPageBuilderAdminHostContext {
    fn eq(&self, other: &Self) -> bool {
        self.controller == other.controller
            && same_optional_arc(&self.facade, &other.facade)
            && same_optional_arc(&self.trait_schemas, &other.trait_schemas)
            && same_optional_arc(&self.contribution_assembly, &other.contribution_assembly)
            && self.editor_capabilities == other.editor_capabilities
            && same_optional_arc(
                &self.editor_capability_evaluation,
                &other.editor_capability_evaluation,
            )
            && self.runtime_context == other.runtime_context
            && same_optional_arc(&self.runtime_scenarios, &other.runtime_scenarios)
            && same_optional_arc(&self.runtime_publish_gate_policy, &other.runtime_publish_gate_policy)
            && self.adapter_workspace == other.adapter_workspace
    }
}

/// Deliver the command effects the shared editor reducer emits. Previously the
/// Dioxus toolbar and workspace discarded Publish/Preview effects, so Save
/// visibly did nothing even when a consumer supplied a working facade.
fn dispatch_facade_effects<F>(
    mut session: Signal<EditorSessionState>,
    facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    effects: Vec<EditorSessionEffect>,
    on_publish_confirmed: F,
)
where
    F: Fn(String, String) + Clone + 'static,
{
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
                    // No request is sent and therefore no completion will ever
                    // arrive to retire an already prepared preview operation.
                    let mut state = session.write();
                    if matches!(request, PageBuilderCapabilityRequest::Preview(_)) {
                        state.preview_in_progress = false;
                        state.preview_request = None;
                    }
                    state.fail("Page Builder facade is missing");
                    continue;
                };
                let capability = request.capability();
                if capability == BuilderCapabilityKind::Preview
                    && facade.provider_status()
                        .is_some_and(|status| !status.preview_enabled())
                {
                    let mut state = session.write();
                    state.preview_in_progress = false;
                    state.preview_request = None;
                    state.fail("Server preview is disabled by the consumer provider or health policy");
                    continue;
                }
                let save_attempt_id = if capability == BuilderCapabilityKind::Publish {
                    // Drop the read guard before taking a write guard to
                    // record the evaluation; temporaries in if-let may live
                    // through the body.
                    let gate_evaluation = { session.read().evaluate_runtime_publish_gate() };
                    if let Some(evaluation) = gate_evaluation {
                        let allowed = evaluation.allowed;
                        let message = crate::core::gate_error_message(&evaluation);
                        let mut state = session.write();
                        state.runtime_publish_gate_evaluation = Some(evaluation);
                        if !allowed {
                            state.fail(message);
                            continue;
                        }
                    }
                    if expected_hash.is_none() {
                        session.write().fail("Publish request has no submitted project hash");
                        continue;
                    }
                    let start = { session.write().mark_save_started_with_id() };
                    match start {
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
                    if state.preview_in_progress {
                        // An already prepared preview still has an in-flight
                        // response. Do not allow a second request to steal its
                        // correlation snapshot or clear its status on completion.
                        state.fail("Preview already in progress");
                        continue;
                    }
                    // Browser-origin previews supply runtime context in the
                    // request instead of calling prepare_server_preview_request.
                    let context = input.runtime.context.clone();
                    let scenario = input.runtime.scenario_id.clone();
                    state.runtime_context = context.clone();
                    state.runtime_context_configured = true;
                    state.active_runtime_scenario = scenario.clone();
                    state.runtime_publish_gate_evaluation = None;
                    state.preview_request = Some((
                        state.controller.editor().revision().project_hash,
                        state.controller.active_page_index(),
                        context,
                        scenario,
                    ));
                    state.preview_in_progress = true;
                }
                let on_publish_confirmed = on_publish_confirmed.clone();
                let submitted_hash = expected_hash.as_ref().map(|hash| hash.hex());
                let _task = spawn(async move {
                    let outcome = facade.execute(request).await;
                    let mut confirmed_write = None;
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
                            match state.handle_publish_response_for_attempt(
                                &response.page_id,
                                &response.revision_id,
                                expected_hash,
                                attempt_id,
                            ) {
                                Ok(()) => {
                                    if let Some(project_hash) = submitted_hash {
                                        confirmed_write =
                                            Some((response.revision_id.clone(), project_hash));
                                    }
                                }
                                Err(error) => {
                                    state.handle_publish_error_for_attempt(attempt_id, error);
                                }
                            }
                        }
                        Ok(PageBuilderCapabilityResponse::Preview(response))
                            if capability == BuilderCapabilityKind::Preview =>
                        {
                            if let Err(error) = state.handle_preview_response(&response) {
                                state.fail(error);
                            }
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
                    drop(state);
                    if let Some((revision, project_hash)) = confirmed_write {
                        on_publish_confirmed(revision, project_hash);
                    }
                });
            }
        }
    }
}

/// A session is owned by an exact page and its server revision. A new
/// owner/revision is a new editor session: reusing the old Dioxus hook storage
/// would leave the previous page's draft and selection mounted.
fn controller_session_key(controller: &AdminCanvasController) -> String {
    // A host may replace its draft without advancing its persisted revision
    // (for example after a remote edit or optimistic reconciler). Reusing
    // use_signal in that case would silently retain the old controlled draft.
    let revision = controller.editor().revision();
    let selected = controller.ui().state.selection.component_id.as_deref().unwrap_or("");
    format!(
        "page:{}:{}|revision:{}|hash:{}|seq:{}|active:{}|selection:{}:{}",
        controller.page_id().len(),
        controller.page_id(),
        controller.revision_id(),
        revision.project_hash.hex(),
        revision.command_sequence,
        controller.active_page_index(),
        selected.len(),
        selected,
    )
}

fn canvas_instance_id(page_id: &str, revision_id: &str, project_hash: &str) -> String {
    fn hex(value: &str) -> String {
        let mut encoded = String::with_capacity(value.len() * 2);
        for byte in value.as_bytes() {
            encoded.push_str(&format!("{byte:02x}"));
        }
        encoded
    }

    format!("fly-canvas-{}-{}-{}", hex(page_id), hex(revision_id), project_hash)
}

fn peer_conflict_blocks_ui_intent(intent: &UiIntent) -> bool {
    matches!(
        intent,
        UiIntent::BeginDrag(_)
            | UiIntent::UpdateHitTest(_)
            | UiIntent::ActivateDropCandidate(_)
            | UiIntent::Drop
            | UiIntent::CutSelection
            | UiIntent::PasteClipboard
            | UiIntent::Execute(_)
            | UiIntent::Undo
            | UiIntent::Redo
            | UiIntent::RequestSave
            | UiIntent::SaveStarted
            | UiIntent::SaveSucceeded { .. }
            | UiIntent::SaveFailed
            | UiIntent::SaveConflict { .. }
    )
}

/// Dioxus PageBuilderAdmin component.
#[component]
pub fn PageBuilderAdmin() -> Element {
    let host_context = use_context::<Option<DioxusPageBuilderAdminHostContext>>();
    let route_context = use_context::<Option<UiRouteContext>>().unwrap_or_default();
    let locale = route_context.locale;

    match host_context {
        Some(context) => {
            let session_key = controller_session_key(&context.controller);
            rsx! {
            PageBuilderAdminWithController {
                key: "{session_key}",
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
    #[props(default = true)]
    pub adapter_workspace: bool,
}

impl PartialEq for PageBuilderAdminWithControllerProps {
    fn eq(&self, other: &Self) -> bool {
        // A document can change repeatedly before its server revision advances.
        // Compare the actual controller state; page ID + revision alone would
        // suppress rerenders of unsaved edits, undo/redo and selection.
        self.controller == other.controller
            && same_optional_arc(&self.facade, &other.facade)
            && same_optional_arc(&self.trait_schemas, &other.trait_schemas)
            && same_optional_arc(&self.contribution_assembly, &other.contribution_assembly)
            && self.editor_capabilities == other.editor_capabilities
            && same_optional_arc(
                &self.editor_capability_evaluation,
                &other.editor_capability_evaluation,
            )
            && self.runtime_context == other.runtime_context
            && same_optional_arc(&self.runtime_scenarios, &other.runtime_scenarios)
            && same_optional_arc(&self.runtime_publish_gate_policy, &other.runtime_publish_gate_policy)
            && self.adapter_workspace == other.adapter_workspace
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
    let host_capabilities = props
        .editor_capability_evaluation
        .as_ref()
        .map(|evaluation| evaluation.effective)
        .or(props.editor_capabilities);
    let provider_status = props.facade.as_ref().and_then(|facade| facade.provider_status());
    let base_capabilities = host_capabilities
        .unwrap_or_else(|| props.controller.ui().editable_capabilities());
    let effective_capabilities = provider_status
        .as_ref()
        .map(|status| status.limit_capabilities(base_capabilities))
        .unwrap_or(base_capabilities)
        .normalized();

    let mut session = use_signal(|| {
        let mut s = EditorSessionState::new(
            props.controller.clone(),
            "Page Builder facade is missing",
            "Page saved successfully",
        );
        if let Some(schemas) = props.trait_schemas.clone() {
            s = s.with_trait_schemas(schemas);
        }
        // The host's tenant/RBAC and provider-health policy narrows the
        // core Fly state machine before any authoring action is accepted.
        if let Err(error) = s.controller.dispatch(
            UiIntent::SetEditableCapabilities(effective_capabilities),
        ) {
            s.fail(format!("Cannot apply editor capabilities: {error}"));
        }
        if let Some(evaluation) = props.editor_capability_evaluation.clone() {
            s = s.with_editor_capability_evaluation(evaluation);
        }
        if let Some(ctx) = props.runtime_context.clone() {
            s = s.with_runtime_context(ctx);
        }
        if let Some(scenarios) = props.runtime_scenarios.clone() {
            s = s.with_runtime_scenarios(scenarios);
        }
        if let Some(policy) = props.runtime_publish_gate_policy.clone() {
            s = s.with_runtime_publish_gate_policy(policy);
        }
        s
    });

    let mut peer_write_conflict = use_signal(|| false);
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let mut peer_write_channel = use_signal(|| None::<BrowserPeerWriteChannel>);

    let session_configuration_key = format!(
        "schemas:{}|evaluation:{}|context:{}|scenarios:{}|policy:{}|facade:{}",
        props
            .trait_schemas
            .as_ref()
            .map(|value| format!("{:p}", Arc::as_ptr(value)))
            .unwrap_or_default(),
        props
            .editor_capability_evaluation
            .as_ref()
            .map(|value| format!("{:p}", Arc::as_ptr(value)))
            .unwrap_or_default(),
        serde_json::to_string(&props.runtime_context).unwrap_or_default(),
        props
            .runtime_scenarios
            .as_ref()
            .map(|value| format!("{:p}", Arc::as_ptr(value)))
            .unwrap_or_default(),
        props
            .runtime_publish_gate_policy
            .as_ref()
            .map(|value| format!("{:p}", Arc::as_ptr(value)))
            .unwrap_or_default(),
        props.facade.is_some(),
    );
    let session_configuration = (
        props.trait_schemas.clone(),
        props.editor_capability_evaluation.clone(),
        props.runtime_context.clone(),
        props.runtime_scenarios.clone(),
        props.runtime_publish_gate_policy.clone(),
        props.facade.is_some(),
    );
    use_effect(use_reactive!(|_session_configuration_key| {
        let (
            trait_schemas,
            editor_capability_evaluation,
            runtime_context,
            runtime_scenarios,
            runtime_publish_gate_policy,
            facade_available,
        ) = session_configuration.clone();
        session.write().update_host_configuration(
            trait_schemas,
            editor_capability_evaluation,
            runtime_context,
            runtime_scenarios,
            runtime_publish_gate_policy,
            facade_available,
        );
    }));

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    {
        let initial_session = session;
        let mut conflict_signal = peer_write_conflict;
        use_effect(move || {
            if peer_write_channel.peek().is_some() {
                return;
            }
            let (page_id, revision_id, project_hash) = {
                let current = initial_session.peek();
                (
                    current.controller.page_id().to_string(),
                    current.controller.revision_id().to_string(),
                    current.controller.editor().revision().project_hash.hex(),
                )
            };
            let on_conflict = move |conflict: BrowserPeerWriteConflict| {
                conflict_signal.set(true);
                let mut state = initial_session.write();
                if state.controller.ui().state.drag.is_some() {
                    if let Err(error) = state.controller.dispatch(UiIntent::CancelDrag) {
                        state.fail(format!(
                            "Another tab saved this page at revision {}. The editor is now read-only; active drag cleanup also failed: {error}",
                            conflict.revision
                        ));
                        return;
                    }
                }
                state.fail(format!(
                    "Another tab saved this page at revision {}. This editor is now read-only; reload to continue.",
                    conflict.revision
                ));
            };
            match BrowserPeerWriteChannel::new(page_id, revision_id, project_hash, on_conflict) {
                Ok(channel) => peer_write_channel.set(Some(channel)),
                Err(error) => tracing::warn!(
                    error = ?error,
                    "Fly cross-tab notifications unavailable; server revision checks remain authoritative"
                ),
            }
        });
    }

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let on_publish_confirmed = {
        let channel = peer_write_channel;
        let conflict = peer_write_conflict;
        move |revision: String, project_hash: String| {
            if *conflict.peek() {
                return;
            }
            if let Some(channel) = channel.peek().as_ref() {
                if let Err(error) = channel.post_confirmed_write(&revision, &project_hash) {
                    tracing::warn!(
                        error = ?error,
                        "Fly save succeeded but cross-tab notification could not be broadcast"
                    );
                }
            }
        }
    };
    #[cfg(not(all(target_arch = "wasm32", feature = "dioxus-web")))]
    let on_publish_confirmed = |_revision: String, _project_hash: String| {};

    // A host may revoke or restore permissions without navigating or saving.
    // Dioxus preserves use_signal state across prop rerenders, so mirror any
    // *changed* effective profile into the live controller as well. Peek to
    // avoid subscribing this effect to every local editor operation.
    use_effect(use_reactive!(|effective_capabilities| {
        let current = session.peek().controller.ui().editable_capabilities();
        if current != effective_capabilities {
            // Drop the write guard before taking another one to report an
            // error; a temporary held by if-let could otherwise overlap.
            let result = {
                session.write().controller.dispatch(
                    UiIntent::SetEditableCapabilities(effective_capabilities),
                )
            };
            if let Err(error) = result {
                session.write().fail(format!("Cannot refresh editor capabilities: {error}"));
            }
        }
    }));

    let current = session.read();
    let page_id = current.controller.page_id().to_string();
    let workspace_document = current.controller.editor().document().clone();
    #[cfg(feature = "dioxus-web")]
    let workspace_runtime_context = current.runtime_context.clone();
    let workspace_registries = current.controller.editor().registries().clone();
    let workspace_page_id = page_id.clone();
    let workspace_selected = current.controller.ui().state.selection.component_id.clone();
    let workspace_initial_page = current.controller.active_page_index();
    let workspace_revision_id = current.controller.revision_id().to_string();
    let workspace_project_hash = current.controller.editor().revision().project_hash.hex();
    let workspace_canvas_instance_id = canvas_instance_id(
        &workspace_page_id,
        &workspace_revision_id,
        &workspace_project_hash,
    );
    let last_error = current.last_error.clone();
    let last_announcement = current.last_announcement.clone();
    let peer_conflict_active = *peer_write_conflict.read();
    let mut capabilities = current.controller.ui().state.capabilities;
    if peer_conflict_active {
        capabilities = CapabilityState::read_only();
    }
    let can_undo = capabilities.history && current.controller.can_undo();
    let can_redo = capabilities.history && current.controller.can_redo();
    let can_save = capabilities.edit
        && capabilities.publish
        && props.facade.is_some()
        && !peer_conflict_active
        && !current.controller.ui().state.dirty.save_in_progress
        && !current.controller.ui().state.dirty.save_conflict
        && !current.controller.ui().state.has_blocking_diagnostics();
    drop(current);

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
    let toolbar_publish_confirmed = on_publish_confirmed.clone();
    #[cfg(feature = "dioxus-web")]
    let canvas_facade = props.facade.clone();
    let on_workspace_error = EventHandler::new(move |error: String| {
        session.write().fail(error);
    });
    let workspace_publish_confirmed = on_publish_confirmed.clone();
    let on_workspace_intent = EventHandler::new(move |envelope: BrowserIntentEnvelope| {
        if *peer_write_conflict.peek() && envelope.is_mutating() {
            session.write().fail(
                "Another tab saved this page. This editor is read-only; reload to continue.",
            );
            return;
        }
        let effects = {
            let mut state = session.write();
            let previous_hash = state.controller.editor().revision().project_hash;
            // The trusted Dioxus callback owns the current host snapshot;
            // typed Fly constructors do not know its revision.
            let mut envelope = envelope;
            envelope.revision = Some(state.controller.revision_id().to_string());
            envelope.project_hash = Some(previous_hash.hex());
            match crate::dispatch_browser_intent(&mut state.controller, envelope) {
                Ok(result) => {
                    // A command from a stale workspace must never dispatch its
                    // asynchronous persistence effect against a newer document.
                    let current_hash = state.controller.editor().revision().project_hash;
                    let current_sequence = state.controller.editor().revision().command_sequence;
                    if result.page_id != state.controller.page_id()
                        || result.revision_id != state.controller.revision_id()
                        || result.project_hash != current_hash.hex()
                        || result.command_sequence != current_sequence
                    {
                        state.fail("Browser workspace response no longer matches the active document");
                        return;
                    }
                    if current_hash != previous_hash {
                        state.runtime_publish_gate_evaluation = None;
                        state.server_preview_html = None;
                        state.preview_request = None;
                        state.preview_in_progress = false;
                    }
                    let mut effects = Vec::new();
                    for effect in result.effects {
                        match effect {
                            BrowserIntentEffect::Announce { message } => {
                                effects.push(EditorSessionEffect::Announce(message));
                            }
                            BrowserIntentEffect::Request {
                                request,
                                expected_hash,
                                command_sequence,
                            } => {
                                if command_sequence != current_sequence {
                                    state.fail("Browser workspace request has a stale command sequence");
                                    return;
                                }
                                let expected_hash = match expected_hash {
                                    Some(hex) if hex == current_hash.hex() => Some(current_hash),
                                    Some(_) => {
                                        state.fail("Browser workspace request has a mismatched project hash");
                                        return;
                                    }
                                    None => None,
                                };
                                effects.push(EditorSessionEffect::ExecuteRequest {
                                    request,
                                    expected_hash,
                                });
                            }
                        }
                    }
                    state.last_error = None;
                    effects
                }
                Err(error) => {
                    state.fail(error.to_string());
                    Vec::new()
                }
            }
        };
        dispatch_facade_effects(
            session,
            workspace_facade.clone(),
            effects,
            workspace_publish_confirmed.clone(),
        );
    });
    #[cfg(feature = "dioxus-web")]
    let canvas_publish_confirmed = on_publish_confirmed.clone();
    #[cfg(feature = "dioxus-web")]
    let on_canvas_ui_intent = EventHandler::new(move |intent: UiIntent| {
        if *peer_write_conflict.peek() && peer_conflict_blocks_ui_intent(&intent) {
            session.write().fail(
                "Another tab saved this page. This editor is read-only; reload to continue.",
            );
            return;
        }
        let effects = session.write().dispatch(EditorSessionAction::DispatchIntent(intent));
        dispatch_facade_effects(
            session,
            canvas_facade.clone(),
            effects,
            canvas_publish_confirmed.clone(),
        );
    });
    #[cfg(feature = "dioxus-web")]
    let workspace_canvas: Option<Element> = {
        let mut canvas_document = workspace_document.clone();
        let active_page = canvas_document
            .project
            .pages
            .get(workspace_initial_page)
            .cloned();
        match active_page {
            Some(active_page) => {
                canvas_document.project.pages = vec![active_page];
                let canvas_nonce = workspace_csp_nonce
                    .as_deref()
                    .and_then(UiCspNonce::new);
                match render_canvas_srcdoc_with_context(
                    &canvas_document,
                    &workspace_canvas_instance_id,
                    &workspace_runtime_context,
                    canvas_nonce.as_ref(),
                ) {
                    Ok(srcdoc) => Some(rsx! {
                        DioxusIsolatedCanvas {
                            srcdoc,
                            instance_id: workspace_canvas_instance_id.clone(),
                            intent_instance_id: workspace_page_id.clone(),
                            can_resize: capabilities.edit && capabilities.styles,
                            session,
                            on_intent: on_workspace_intent,
                            on_ui_intent: on_canvas_ui_intent,
                            on_error: Some(on_workspace_error),
                        }
                    }),
                    Err(error) => Some(rsx! {
                        div {
                            class: "rounded-md border border-destructive/40 bg-destructive/10 p-4 text-sm",
                            role: "alert",
                            "Canvas rendering failed: {error}"
                        }
                    })
                }
            }
            None => Some(rsx! {
                div {
                    class: "rounded-md border border-destructive/40 bg-destructive/10 p-4 text-sm",
                    role: "alert",
                    "The active page is unavailable for isolated canvas rendering."
                }
            })
        }
    };
    #[cfg(not(feature = "dioxus-web"))]
    let workspace_canvas: Option<Element> = None;

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
                            if *peer_write_conflict.peek() {
                                return;
                            }
                            session.write().dispatch(EditorSessionAction::DispatchIntent(UiIntent::Undo));
                        },
                        "Undo"
                    }
                    button {
                        class: "px-3 py-1.5 text-sm rounded bg-muted hover:bg-muted/80 disabled:opacity-50",
                        disabled: !can_redo,
                        onclick: move |_| {
                            if *peer_write_conflict.peek() {
                                return;
                            }
                            session.write().dispatch(EditorSessionAction::DispatchIntent(UiIntent::Redo));
                        },
                        "Redo"
                    }
                    button {
                        class: "px-3 py-1.5 text-sm rounded bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50",
                        disabled: !can_save,
                        onclick: move |_| {
                            if *peer_write_conflict.peek() {
                                return;
                            }
                            let effects = session.write().dispatch(
                                EditorSessionAction::DispatchIntent(UiIntent::RequestSave)
                            );
                            dispatch_facade_effects(
                                session,
                                toolbar_facade.clone(),
                                effects,
                                toolbar_publish_confirmed.clone(),
                            );
                        },
                        "Save"
                    }
                }
            }

            if peer_conflict_active {
                div {
                    class: "p-3 bg-amber-500/10 text-amber-900 dark:text-amber-200 text-sm border-b border-amber-500/30",
                    role: "alert",
                    "Another tab saved this page. This editor is now read-only; reload to continue from the latest revision."
                }
            } else if let Some(err) = last_error {
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
                            registries: Some(workspace_registries.clone()),
                            contribution_assembly: workspace_assembly.clone(),
                            instance_id: Some(workspace_page_id.clone()),
                            on_intent: Some(on_workspace_intent),
                            on_change: None,
                            on_error: Some(on_workspace_error),
                            messages: workspace_messages.clone(),
                            capabilities: Some(capabilities),
                            initial_page: Some(workspace_initial_page),
                            selected_component_id: workspace_selected.clone(),
                            host_can_undo: Some(can_undo),
                            host_can_redo: Some(can_redo),
                            csp_nonce: workspace_csp_nonce.clone(),
                            canvas: workspace_canvas,
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

#[cfg(test)]
mod props_regression_tests {
    use super::*;
    use fly::{ComponentPatch, EditorCommand};
    use serde_json::json;

    #[test]
    fn props_detect_draft_changes_without_a_new_server_revision() {
        let controller = AdminCanvasController::new(
            "home", "rev-1", json!({
                "pages": [{
                    "id": "home",
                    "component": { "id": "root", "type": "wrapper", "components": [{
                        "id": "hero", "type": "text", "content": "one"
                    }] }
                }]
            }),
        ).expect("controller");
        let original = DioxusPageBuilderAdminHostContext::new(controller);
        let mut changed = original.clone();
        changed.controller.dispatch(UiIntent::execute(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().set_field("content", json!("two")),
        })).expect("edit");
        assert_eq!(original.controller.revision_id(), changed.controller.revision_id());
        assert!(original != changed, "draft edit must trigger prop update");
        let mut workspace = original.clone();
        workspace.adapter_workspace = false;
        assert!(original != workspace, "mount mode must trigger prop update");
    }

    #[test]
    fn restricted_host_capability_disables_editing_and_saving() {
        let mut controller = AdminCanvasController::new(
            "home",
            "rev-1",
            json!({ "pages": [{
                "id": "home",
                "component": { "id": "root", "type": "wrapper" }
            }] }),
        ).expect("controller");
        controller.dispatch(UiIntent::SetEditableCapabilities(CapabilityState::read_only()))
            .expect("apply the host restriction");
        assert!(!controller.ui().state.capabilities.edit);
        assert!(!controller.ui().state.capabilities.history);
        assert!(controller.dispatch(UiIntent::RequestSave).is_err());
        assert!(controller.dispatch(UiIntent::Undo).is_err());
    }

    #[test]
    fn canvas_iframe_identity_tracks_document_revision_and_hash() {
        let first = canvas_instance_id("home", "rev-1", "hash-1");
        assert_ne!(first, canvas_instance_id("home", "rev-1", "hash-2"));
        assert_ne!(first, canvas_instance_id("home", "rev-2", "hash-1"));
        assert_ne!(first, canvas_instance_id("about", "rev-1", "hash-1"));
        assert!(first.starts_with("fly-canvas-"));
    }

    #[test]
    fn peer_write_fence_blocks_mutations_but_allows_selection_and_cancel() {
        assert!(peer_conflict_blocks_ui_intent(&UiIntent::Drop));
        assert!(peer_conflict_blocks_ui_intent(&UiIntent::Undo));
        assert!(!peer_conflict_blocks_ui_intent(&UiIntent::Select(Some("hero".to_string()))));
        assert!(!peer_conflict_blocks_ui_intent(&UiIntent::CancelDrag));
    }

    #[test]
    fn session_keys_are_scoped_to_page_and_revision() {
        let doc = json!({
            "pages": [{ "id": "home", "component": { "id": "root", "type": "wrapper" } }]
        });
        let home_v1 = AdminCanvasController::new("home", "rev-1", doc.clone())
            .expect("first page");
        let home_v2 = AdminCanvasController::new("home", "rev-2", doc.clone())
            .expect("second revision");
        let other = AdminCanvasController::new("other", "rev-1", doc)
            .expect("second page");
        assert_ne!(controller_session_key(&home_v1), controller_session_key(&home_v2));
        assert_ne!(controller_session_key(&home_v1), controller_session_key(&other));

        let mut changed_draft = home_v1.clone();
        changed_draft.dispatch(UiIntent::execute(EditorCommand::Patch {
            component_id: "root".to_string(),
            patch: ComponentPatch::default().set_attribute(
                "aria-label", json!("Updated draft without a new server revision"),
            ),
        })).expect("patch existing root");
        assert_eq!(changed_draft.revision_id(), home_v1.revision_id());
        assert_ne!(controller_session_key(&home_v1), controller_session_key(&changed_draft));

        let mut selected = home_v1.clone();
        selected.dispatch(UiIntent::Select(Some("root".to_string())))
            .expect("select root");
        assert_ne!(controller_session_key(&home_v1), controller_session_key(&selected));
    }
}
