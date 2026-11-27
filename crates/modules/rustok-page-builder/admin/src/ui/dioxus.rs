#[cfg(feature = "dioxus-web")]
use super::dioxus_canvas::DioxusIsolatedCanvas;
use super::dioxus_recovery::{DraftRecoveryBase, DraftRecoveryStatus, initialize_draft_recovery};
use crate::BrowserIntentEffect;
use crate::core::session::{EditorSessionAction, EditorSessionEffect, EditorSessionState};
#[cfg(feature = "dioxus-web")]
use crate::editor::render_canvas_srcdoc_with_context;
use crate::i18n::t;
use crate::model::AdminCanvasController;
use crate::transport::PageBuilderAdminFacade;
use dioxus::prelude::*;
use fly::{GrapesJsCodec, TraitSchemaRegistry};
use fly_browser::BrowserIntentEnvelope;
use fly_dioxus::{FlyEditorWorkspace, FlyFullEditor};
use fly_ui::{
    CapabilityState, ContributionAssemblyResult, EditorCapabilityEvaluation, EditorMessages,
    UiIntent,
};
use rustok_page_builder::dto::{
    BuilderCapabilityKind, PageBuilderCapabilityRequest, PageBuilderCapabilityResponse,
    PublishPageBuilderInput,
};
use rustok_page_builder::{RuntimeContextScenario, RuntimePublishGatePolicy};
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
    /// Host-owned refresh action. It must load the latest server document and replace
    /// the controller; the editor exposes the local draft before invoking this callback.
    pub on_reload_requested: Option<EventHandler<()>>,
    /// Stable opaque tenant/user scope used to isolate browser-local draft recovery.
    /// Do not pass an email address or other directly identifying value.
    pub draft_recovery_scope: Option<String>,
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
            on_reload_requested: None,
            draft_recovery_scope: None,
        }
    }

    /// Mounts the shared Fly adapter workspace. Enabled by default; hosts may
    /// explicitly opt out when they need the minimal placeholder shell.
    pub fn with_adapter_workspace(mut self, enabled: bool) -> Self {
        self.adapter_workspace = enabled;
        self
    }

    /// Attach the consumer's authoritative reload action for revision conflicts.
    pub fn on_reload_requested(mut self, handler: EventHandler<()>) -> Self {
        self.on_reload_requested = Some(handler);
        self
    }

    /// Configure an opaque, stable scope for the authenticated tenant and actor.
    /// A different value is required after account or tenant changes.
    pub fn with_draft_recovery_scope(mut self, scope: impl Into<String>) -> Self {
        self.draft_recovery_scope = Some(scope.into());
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
            && same_optional_arc(
                &self.runtime_publish_gate_policy,
                &other.runtime_publish_gate_policy,
            )
            && self.adapter_workspace == other.adapter_workspace
            && self.on_reload_requested == other.on_reload_requested
            && self.draft_recovery_scope == other.draft_recovery_scope
    }
}

/// Deliver the command effects the shared editor reducer emits. Previously the
/// Dioxus toolbar and workspace discarded Publish/Preview effects, so Save
/// visibly did nothing even when a consumer supplied a working facade.
// An ambiguous save may only be retried with its exact original server request.
// A hash or revision alone cannot identify the submitted document.
fn same_publish_input(left: &PublishPageBuilderInput, right: &PublishPageBuilderInput) -> bool {
    left.page_id == right.page_id
        && left.revision_id == right.revision_id
        && left.project_data == right.project_data
}

// A copy confirmation belongs to the exact displayed working revision.
fn draft_stamp(controller: &AdminCanvasController) -> (fly::ProjectHash, u64) {
    let revision = controller.editor().revision();
    (revision.project_hash, revision.command_sequence)
}

fn draft_reload_safe(
    controller: &AdminCanvasController,
    persisted: bool,
    copied: Option<(fly::ProjectHash, u64)>,
) -> bool {
    persisted || copied == Some(draft_stamp(controller))
}

fn can_retry_unknown_save(
    controller: &AdminCanvasController,
    submitted: &PublishPageBuilderInput,
) -> bool {
    controller.page_id() == submitted.page_id
        && controller.revision_id() == submitted.revision_id
        && GrapesJsCodec::encode_value(controller.editor().document())
            .is_ok_and(|data| data == submitted.project_data)
}

fn dispatch_facade_effects<F>(
    mut session: Signal<EditorSessionState>,
    facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    effects: Vec<EditorSessionEffect>,
    on_publish_confirmed: F,
    mut save_outcome_unknown: Signal<Option<PublishPageBuilderInput>>,
) where
    F: Fn(String, String, Value) + Clone + 'static,
{
    for effect in effects {
        match effect {
            EditorSessionEffect::Announce(message) => {
                session.write().announce(message);
            }
            EditorSessionEffect::SaveAttemptStarted(_) => {}
            EditorSessionEffect::ExecuteRequest {
                request,
                expected_hash,
                command_sequence,
                preview_attempt_id,
            } => {
                let preview_attempt_id =
                    if let PageBuilderCapabilityRequest::Preview(input) = &request {
                        let mut state = session.write();
                        if let Some(id) = preview_attempt_id {
                            if !state.preview_attempt_active(id) {
                                continue;
                            }
                        } else if state.preview_in_progress {
                            state.fail("Preview already in progress");
                            continue;
                        }
                        if let Err(error) = state
                            .controller
                            .validate_preview_request(input, command_sequence)
                        {
                            if let Some(id) = preview_attempt_id {
                                state.handle_preview_failure(id, error.to_string());
                            } else {
                                state.fail(error.to_string());
                            }
                            continue;
                        }
                        if let Some(id) = preview_attempt_id {
                            Some(id)
                        } else {
                            let context = input.runtime.context.clone();
                            let scenario = input.runtime.scenario_id.clone();
                            if !state.runtime_context_configured
                                || state.runtime_context != context
                                || state.active_runtime_scenario != scenario
                            {
                                state.runtime_publish_gate_evaluation = None;
                                state.server_preview_html = None;
                            }
                            state.runtime_context = context.clone();
                            state.runtime_context_configured = true;
                            state.active_runtime_scenario = scenario.clone();
                            let attempt = crate::core::session::PreviewAttempt::new(
                                state.controller.editor().revision().project_hash,
                                state.controller.active_page_index(),
                                context,
                                scenario,
                            );
                            let id = attempt.id;
                            state.preview_request = Some(attempt);
                            state.preview_in_progress = true;
                            Some(id)
                        }
                    } else {
                        None
                    };
                let Some(facade) = facade.clone() else {
                    // No request is sent and therefore no completion will ever
                    // arrive to retire an already prepared preview operation.
                    let mut state = session.write();
                    if let Some(id) = preview_attempt_id {
                        state.handle_preview_failure(
                            id,
                            "Page Builder facade is missing".to_string(),
                        );
                    } else {
                        state.fail("Page Builder facade is missing");
                    }
                    continue;
                };
                let capability = request.capability();
                let submitted_input = match &request {
                    PageBuilderCapabilityRequest::Publish(input) => {
                        let matches_unknown = save_outcome_unknown
                            .peek()
                            .as_ref()
                            .is_none_or(|submitted| same_publish_input(submitted, input));
                        if !matches_unknown {
                            session.write().fail(
                                "The working draft differs from the unknown save; preserve it and reload before saving.",
                            );
                            continue;
                        }
                        Some(input.clone())
                    }
                    _ => None,
                };
                if capability == BuilderCapabilityKind::Preview
                    && facade
                        .provider_status()
                        .is_some_and(|status| !status.preview_enabled())
                {
                    let mut state = session.write();
                    if let Some(id) = preview_attempt_id {
                        state.handle_preview_failure(
                            id,
                            "Server preview is disabled by the consumer provider or health policy"
                                .to_string(),
                        );
                    }
                    continue;
                }
                let save_attempt = if capability == BuilderCapabilityKind::Publish {
                    if expected_hash.is_none() {
                        session
                            .write()
                            .fail("Save request has no submitted project hash");
                        continue;
                    }
                    let start = {
                        session.write().controller.begin_save_request(
                            &request,
                            expected_hash,
                            command_sequence,
                        )
                    };
                    match start {
                        Ok(attempt) => Some(attempt),
                        Err(error) => {
                            session.write().fail(error.to_string());
                            continue;
                        }
                    }
                } else {
                    None
                };
                let on_publish_confirmed = on_publish_confirmed.clone();
                let submitted_hash = expected_hash.as_ref().map(|hash| hash.hex());
                let _task = spawn(async move {
                    let outcome = facade.execute(request).await;
                    let mut confirmed_write: Option<(String, String, Value)> = None;
                    let mut state = session.write();
                    if preview_attempt_id.is_some_and(|id| !state.preview_attempt_active(id)) {
                        return;
                    }
                    match outcome {
                        Ok(PageBuilderCapabilityResponse::Publish(response))
                            if capability == BuilderCapabilityKind::Publish =>
                        {
                            let Some(attempt) = save_attempt.as_ref() else {
                                return;
                            };
                            if state.controller.active_save_attempt().as_ref() != Some(attempt) {
                                return;
                            }
                            match state.handle_publish_response_for_attempt(
                                &response.page_id,
                                &response.revision_id,
                                expected_hash,
                                attempt,
                            ) {
                                Ok(()) => {
                                    save_outcome_unknown.set(None);
                                    if let (Some(project_hash), Some(input)) =
                                        (submitted_hash, submitted_input)
                                    {
                                        confirmed_write = Some((
                                            response.revision_id.clone(),
                                            project_hash,
                                            input.project_data,
                                        ));
                                    }
                                }
                                Err(error) => {
                                    state.handle_publish_error_for_attempt(attempt, error);
                                }
                            }
                        }
                        Ok(PageBuilderCapabilityResponse::Preview(response))
                            if capability == BuilderCapabilityKind::Preview =>
                        {
                            if let Some(id) = preview_attempt_id
                                && let Err(error) = state.handle_preview_response(id, &response)
                            {
                                state.fail(error);
                            }
                        }
                        Ok(other) => {
                            let error = format!(
                                "Page Builder facade returned `{}` for a `{capability}` request",
                                other.capability()
                            );
                            if let Some(id) = save_attempt {
                                // A malformed retry response does not resolve the
                                // original ambiguous write.
                                state.handle_publish_error_for_attempt(&id, error);
                            } else if let Some(id) = preview_attempt_id {
                                state.handle_preview_failure(id, error);
                            } else {
                                state.fail(error);
                            }
                        }
                        Err(error) => {
                            if let Some(id) = save_attempt {
                                match error.stable_code.as_deref() {
                                    Some("REVISION_CONFLICT") => {
                                        if state.handle_publish_conflict_for_attempt(
                                            &id,
                                            error.to_string(),
                                        ) {
                                            save_outcome_unknown.set(None);
                                        }
                                    }
                                    Some("PAGE_DOCUMENT_SAVE_OUTCOME_UNKNOWN") => {
                                        if state.handle_publish_error_for_attempt(
                                            &id,
                                            error.to_string(),
                                        ) {
                                            save_outcome_unknown.set(submitted_input);
                                        }
                                    }
                                    _ => {
                                        // A generic retry failure cannot establish
                                        // whether the earlier request committed.
                                        state.handle_publish_error_for_attempt(
                                            &id,
                                            error.to_string(),
                                        );
                                    }
                                }
                            } else if let Some(id) = preview_attempt_id {
                                state.handle_preview_failure(id, error.to_string());
                            } else {
                                state.fail(error.to_string());
                            }
                        }
                    }
                    drop(state);
                    if let Some((revision, project_hash, project_data)) = confirmed_write {
                        on_publish_confirmed(revision, project_hash, project_data);
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
    let selected = controller
        .ui()
        .state
        .selection
        .component_id
        .as_deref()
        .unwrap_or("");
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

/// Changes after every document command, including undo/redo to a previous content hash.
pub(super) fn canvas_instance_id(
    page_id: &str,
    active_page_index: usize,
    revision_id: &str,
    project_hash: &str,
    command_sequence: u64,
) -> String {
    fn hex(value: &str) -> String {
        let mut encoded = String::with_capacity(value.len() * 2);
        for byte in value.as_bytes() {
            encoded.push_str(&format!("{byte:02x}"));
        }
        encoded
    }

    format!(
        "fly-canvas-{}-{}-{}-{}-{}",
        hex(page_id),
        active_page_index,
        hex(revision_id),
        project_hash,
        command_sequence
    )
}

fn write_conflict_active(peer_write_conflict: bool, controller: &AdminCanvasController) -> bool {
    peer_write_conflict || controller.ui().state.dirty.save_conflict
}

fn local_recovery_json(controller: &AdminCanvasController) -> String {
    match GrapesJsCodec::encode_value(controller.editor().document()) {
        Ok(project) => serde_json::to_string_pretty(&project)
            .unwrap_or_else(|error| format!("Could not serialize local draft: {error}")),
        Err(error) => format!("Could not serialize local draft: {error}"),
    }
}

fn bind_workspace_intent_snapshot(
    mut envelope: BrowserIntentEnvelope,
    page_id: &str,
    revision_id: &str,
    project_hash: &str,
    command_sequence: u64,
) -> BrowserIntentEnvelope {
    envelope.page_id = Some(page_id.to_string());
    envelope.revision = Some(revision_id.to_string());
    envelope.project_hash = Some(project_hash.to_string());
    envelope.command_sequence = Some(command_sequence);
    envelope
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
            | UiIntent::SaveStarted { .. }
            | UiIntent::SaveSucceeded { .. }
            | UiIntent::SaveFailed { .. }
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
            rsx! {
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
                on_reload_requested: context.on_reload_requested,
                draft_recovery_scope: context.draft_recovery_scope,
            }
            }
        }
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
    #[props(default)]
    pub on_reload_requested: Option<EventHandler<()>>,
    #[props(default)]
    pub draft_recovery_scope: Option<String>,
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
            && same_optional_arc(
                &self.runtime_publish_gate_policy,
                &other.runtime_publish_gate_policy,
            )
            && self.adapter_workspace == other.adapter_workspace
            && self.on_reload_requested == other.on_reload_requested
            && self.draft_recovery_scope == other.draft_recovery_scope
    }
}

/// Dioxus PageBuilderAdminWithController component.
///
/// The inner session is keyed by the incoming controller identity so direct consumers receive
/// the same page/revision reset semantics as the context-backed PageBuilderAdmin entrypoint.
#[component]
pub fn PageBuilderAdminWithController(props: PageBuilderAdminWithControllerProps) -> Element {
    let scope_key = props
        .draft_recovery_scope
        .as_deref()
        .map(|scope| fly::sha256_hex(scope.as_bytes()))
        .unwrap_or_else(|| "recovery-disabled".to_string());
    let session_key = format!(
        "{}|scope:{}",
        controller_session_key(&props.controller),
        scope_key
    );
    rsx! {
        PageBuilderAdminWithControllerSession {
            key: "{session_key}",
            inner: props
        }
    }
}

#[component]
fn PageBuilderAdminWithControllerSession(inner: PageBuilderAdminWithControllerProps) -> Element {
    let props = inner;
    let route_context = use_context::<Option<UiRouteContext>>().unwrap_or_default();
    let locale = route_context.locale;
    // The workspace renders the project stylesheet inline; without the request nonce the host's
    // policy drops it and the canvas renders unstyled.
    let workspace_csp_nonce = try_consume_context::<Option<UiCspNonce>>()
        .flatten()
        .or_else(|| try_consume_context::<UiCspNonce>())
        .map(|nonce| nonce.as_str().to_string());
    let host_capabilities = props
        .editor_capability_evaluation
        .as_ref()
        .map(|evaluation| evaluation.effective)
        .or(props.editor_capabilities);
    let provider_status = props
        .facade
        .as_ref()
        .and_then(|facade| facade.provider_status());
    let base_capabilities =
        host_capabilities.unwrap_or_else(|| props.controller.ui().editable_capabilities());
    let effective_capabilities = provider_status
        .as_ref()
        .map(|status| status.limit_capabilities(base_capabilities))
        .unwrap_or(base_capabilities)
        .normalized();

    let recovery_bootstrap = use_hook(|| {
        initialize_draft_recovery(props.draft_recovery_scope.as_deref(), &props.controller)
    });
    let mut initial_recovery_status = recovery_bootstrap.status.clone();
    let mut initial_recovery_conflict_json = recovery_bootstrap.conflict_json.clone();
    let initial_session = {
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
        if let Err(error) = s
            .controller
            .dispatch(UiIntent::SetEditableCapabilities(effective_capabilities))
        {
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
        if let Some(merged_document) = recovery_bootstrap.merged_document.clone() {
            let preserved_local_document = merged_document.clone();
            if let Err(error) = s.controller.apply_recovered_merge(merged_document) {
                initial_recovery_conflict_json =
                    serde_json::to_string_pretty(&preserved_local_document).ok();
                initial_recovery_status = DraftRecoveryStatus::Error(format!(
                    "Could not apply the recovered draft merge: {error}"
                ));
                s.fail(format!(
                    "Could not apply the recovered draft merge: {error}"
                ));
            }
        }
        s
    };
    let mut recovery_status = use_signal(|| initial_recovery_status.clone());
    let mut recovery_conflict_json = use_signal(|| initial_recovery_conflict_json.clone());
    let recovery_base = use_signal(|| recovery_bootstrap.base.clone());
    let recovery_store = recovery_bootstrap.store.clone();
    let recovery_consumed_records = recovery_bootstrap.consumed_records.clone();
    let mut recovery_persisted = use_signal(|| recovery_bootstrap.persisted);
    let mut session = use_signal(move || initial_session);

    let mut peer_write_conflict = use_signal(|| false);
    // A timeout with no visible receipt is not a failed save: only an exact retry
    // or an authoritative refresh can resolve it. Keep the editor read-only meanwhile.
    let mut save_outcome_unknown = use_signal(|| None::<PublishPageBuilderInput>);
    let mut local_draft_copied = use_signal(|| None::<(fly::ProjectHash, u64)>);
    // Separate editor roots in the same browser document are independent writers.
    // The document nonce added by BrowserPeerWriteChannel keeps this ID unique across tabs.
    let peer_write_scope_id = use_hook(|| uuid::Uuid::new_v4().to_string());
    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    let mut peer_write_channel = use_signal(|| None::<BrowserPeerWriteChannel>);

    let session_configuration_key = format!(
        "schemas:{}|evaluation:{}|context:{}|scenarios:{}|policy:{}|facade:{}|recovery_scope:{}",
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
        props
            .draft_recovery_scope
            .as_deref()
            .map(|scope| fly::sha256_hex(scope.as_bytes()))
            .unwrap_or_default(),
    );
    let session_configuration = (
        props.trait_schemas.clone(),
        props.editor_capability_evaluation.clone(),
        props.runtime_context.clone(),
        props.runtime_scenarios.clone(),
        props.runtime_publish_gate_policy.clone(),
        props.facade.is_some(),
        props.draft_recovery_scope.clone(),
    );
    use_effect(use_reactive!(|session_configuration_key| {
        let _ = &session_configuration_key;
        let (
            trait_schemas,
            editor_capability_evaluation,
            runtime_context,
            runtime_scenarios,
            runtime_publish_gate_policy,
            facade_available,
            _draft_recovery_scope,
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

    {
        let persistence_store = recovery_store.clone();
        let persistence_base = recovery_base;
        let mut persistence_status = recovery_status;
        let mut persistence_ready = recovery_persisted;
        use_effect(move || {
            let Some(store) = persistence_store.as_ref() else {
                persistence_ready.set(false);
                return;
            };
            let (dirty, project_data) = {
                let state = session.read();
                let dirty = state.controller.editor().revision().dirty;
                let project_data = if dirty {
                    GrapesJsCodec::encode_value(state.controller.editor().document()).ok()
                } else {
                    None
                };
                (dirty, project_data)
            };
            if matches!(
                &*persistence_status.peek(),
                DraftRecoveryStatus::Conflict { .. } | DraftRecoveryStatus::Error(_)
            ) {
                // Conflicts and unreadable recovery records are preserved until explicitly resolved.
                return;
            }
            persistence_ready.set(false);
            if !dirty {
                match store.clear_current() {
                    Ok(()) => {
                        persistence_ready.set(true);
                        if matches!(
                            &*persistence_status.peek(),
                            DraftRecoveryStatus::Restored { .. }
                                | DraftRecoveryStatus::Merged { .. }
                                | DraftRecoveryStatus::SaveError(_)
                        ) {
                            persistence_status.set(DraftRecoveryStatus::Ready);
                        }
                    }
                    Err(error) => {
                        persistence_status.set(DraftRecoveryStatus::SaveError(error));
                    }
                }
                return;
            }

            let Some(project_data) = project_data else {
                persistence_status.set(DraftRecoveryStatus::SaveError(
                    "Could not serialize the current draft for local recovery.".to_string(),
                ));
                return;
            };
            let base = (*persistence_base.read()).clone();
            match store.save(&base, project_data) {
                Ok(()) => {
                    persistence_ready.set(true);
                    if matches!(
                        &*persistence_status.peek(),
                        DraftRecoveryStatus::SaveError(_)
                    ) {
                        persistence_status.set(DraftRecoveryStatus::Ready);
                    }
                }
                Err(error) => {
                    persistence_status.set(DraftRecoveryStatus::SaveError(error));
                }
            }
        });
    }

    #[cfg(all(target_arch = "wasm32", feature = "dioxus-web"))]
    {
        let initial_session = session;
        let initial_recovery_base = recovery_base;
        let mut conflict_signal = peer_write_conflict;
        use_effect(move || {
            if peer_write_channel.peek().is_some() {
                return;
            }
            let (page_id, revision_id, project_hash) = {
                let current = initial_session.peek();
                let base = initial_recovery_base.peek();
                let base_hash = GrapesJsCodec::decode_value(base.project_data.clone())
                    .map(|document| document.hash().hex())
                    .unwrap_or_else(|_| current.controller.editor().revision().project_hash.hex());
                (
                    current.controller.page_id().to_string(),
                    base.revision_id.clone(),
                    base_hash,
                )
            };
            let on_conflict = move |conflict: BrowserPeerWriteConflict| {
                conflict_signal.set(true);
                let mut state = initial_session.write();
                let drag_cleanup_error = if state.controller.ui().state.drag.is_some() {
                    state.controller.dispatch(UiIntent::CancelDrag).err()
                } else {
                    None
                };
                // This also retires any in-flight save attempt. A late facade response can no
                // longer acknowledge an older revision after this peer write is observed.
                state.handle_peer_write_conflict(conflict.revision.clone());
                if let Some(error) = drag_cleanup_error {
                    state.fail(format!(
                        "Another tab saved this page at revision {}. The editor is read-only; active drag cleanup also failed: {error}",
                        conflict.revision
                    ));
                }
            };
            match BrowserPeerWriteChannel::new_scoped(
                page_id,
                revision_id,
                project_hash,
                peer_write_scope_id.clone(),
                on_conflict,
            ) {
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
        let base = recovery_base;
        let store = recovery_store.clone();
        let consumed_records = recovery_consumed_records.clone();
        let status = recovery_status;
        let persisted = recovery_persisted;
        let current_session = session;
        move |revision: String, project_hash: String, submitted_project_data: Value| {
            let mut base = base;
            let mut status = status;
            let mut persisted = persisted;
            base.set(DraftRecoveryBase {
                revision_id: revision.clone(),
                project_data: submitted_project_data,
            });
            if let Some(store) = store.as_ref() {
                if matches!(&*status.peek(), DraftRecoveryStatus::Error(_)) {
                    // The server save is confirmed, but a fatal/unreadable local record remains.
                    // Do not consume or clear any recovery records without explicit user action.
                    persisted.set(false);
                } else {
                    let mut cleanup_error = store.clear_if_unchanged(&consumed_records).err();
                    if !current_session.read().controller.editor().revision().dirty {
                        if let Err(error) = store.clear_current() {
                            cleanup_error.get_or_insert(error);
                        }
                        if let Some(error) = cleanup_error {
                            status.set(DraftRecoveryStatus::SaveError(error));
                            persisted.set(false);
                        } else {
                            status.set(DraftRecoveryStatus::Ready);
                            persisted.set(true);
                        }
                    } else {
                        if let Some(error) = cleanup_error {
                            status.set(DraftRecoveryStatus::SaveError(error));
                        }
                        persisted.set(false);
                    }
                }
            }
            if !*conflict.peek() {
                if let Some(channel) = channel.peek().as_ref() {
                    if let Err(error) = channel.post_confirmed_write(&revision, &project_hash) {
                        tracing::warn!(
                            error = ?error,
                            "Fly save succeeded but cross-tab notification could not be broadcast"
                        );
                    }
                }
            }
        }
    };
    #[cfg(not(all(target_arch = "wasm32", feature = "dioxus-web")))]
    let on_publish_confirmed = {
        let base = recovery_base;
        move |revision: String, _project_hash: String, submitted_project_data: Value| {
            let mut base = base;
            base.set(DraftRecoveryBase {
                revision_id: revision,
                project_data: submitted_project_data,
            });
        }
    };

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
                session
                    .write()
                    .controller
                    .dispatch(UiIntent::SetEditableCapabilities(effective_capabilities))
            };
            if let Err(error) = result {
                session
                    .write()
                    .fail(format!("Cannot refresh editor capabilities: {error}"));
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
    let workspace_revision = current.controller.editor().revision();
    let workspace_project_hash = workspace_revision.project_hash.hex();
    let workspace_command_sequence = workspace_revision.command_sequence;
    let workspace_canvas_instance_id = canvas_instance_id(
        &workspace_page_id,
        workspace_initial_page,
        &workspace_revision_id,
        &workspace_project_hash,
        workspace_command_sequence,
    );
    let last_error = current.last_error.clone();
    let last_announcement = current.last_announcement.clone();
    // A server-side CAS conflict and a BroadcastChannel notice both require the
    // same recovery path. The former does not set the browser-only peer signal.
    let peer_conflict_active =
        write_conflict_active(*peer_write_conflict.read(), &current.controller);
    let recovery_status_value = (*recovery_status.read()).clone();
    let recovery_conflict_active =
        matches!(&recovery_status_value, DraftRecoveryStatus::Conflict { .. });
    let recovery_storage_unavailable = matches!(
        &recovery_status_value,
        DraftRecoveryStatus::Disabled(_)
            | DraftRecoveryStatus::Error(_)
            | DraftRecoveryStatus::SaveError(_)
    );
    let displayed_draft_stamp = draft_stamp(&current.controller);
    let copied_current_draft = *local_draft_copied.read() == Some(displayed_draft_stamp);
    let can_reload_draft = draft_reload_safe(
        &current.controller,
        *recovery_persisted.read(),
        *local_draft_copied.read(),
    );
    let save_outcome_unknown_active = save_outcome_unknown.read().is_some();
    let unknown_save_retry_matches = save_outcome_unknown
        .read()
        .as_ref()
        .is_some_and(|submitted| can_retry_unknown_save(&current.controller, submitted));
    let local_recovery_json =
        (peer_conflict_active || save_outcome_unknown_active || recovery_storage_unavailable)
            .then(|| local_recovery_json(&current.controller));
    let local_recovery_json_text = local_recovery_json.as_deref().unwrap_or_default();
    let recovery_conflict_json_text = (*recovery_conflict_json.read()).clone().unwrap_or_default();
    let save_retry_in_progress = current.controller.ui().state.dirty.save_in_progress
        || current
            .controller
            .ui()
            .state
            .dirty
            .save_started_request_id
            .is_some();
    let mut capabilities = current.controller.ui().state.capabilities;
    if peer_conflict_active || save_outcome_unknown_active || recovery_conflict_active {
        capabilities = CapabilityState::read_only();
    }
    let can_undo = capabilities.history && current.controller.can_undo();
    let can_redo = capabilities.history && current.controller.can_redo();
    let can_save = capabilities.edit
        && props.facade.is_some()
        && !peer_conflict_active
        && !recovery_conflict_active
        && !current.controller.ui().state.dirty.save_in_progress
        && current
            .controller
            .ui()
            .state
            .dirty
            .save_started_request_id
            .is_none()
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
    let intent_page_id = workspace_page_id.clone();
    let intent_revision_id = workspace_revision_id.clone();
    let intent_project_hash = workspace_project_hash.clone();
    let intent_command_sequence = workspace_command_sequence;
    let on_workspace_intent = EventHandler::new(move |envelope: BrowserIntentEnvelope| {
        let write_conflict = *peer_write_conflict.peek()
            || save_outcome_unknown.peek().is_some()
            || matches!(
                &*recovery_status.peek(),
                DraftRecoveryStatus::Conflict { .. }
            )
            || session.read().controller.ui().state.dirty.save_conflict;
        if write_conflict && envelope.is_mutating() {
            session
                .write()
                .fail("This editor is read-only until the save result is reconciled.");
            return;
        }
        if envelope.is_mutating() {
            recovery_persisted.set(false);
        }
        let effects = {
            let mut state = session.write();
            let previous_hash = state.controller.editor().revision().project_hash;
            let previous_active_page_index = state.controller.active_page_index();
            // The callback must carry the snapshot that rendered its workspace. Reading the
            // live controller here would silently upgrade a stale event to the latest revision.
            let envelope = bind_workspace_intent_snapshot(
                envelope,
                &intent_page_id,
                &intent_revision_id,
                &intent_project_hash,
                intent_command_sequence,
            );
            match crate::dispatch_browser_intent(&mut state.controller, envelope) {
                Ok(result) => {
                    // A command from a stale workspace must never dispatch its
                    // asynchronous persistence effect against a newer document.
                    let current_hash = state.controller.editor().revision().project_hash;
                    let current_active_page_index = state.controller.active_page_index();
                    let current_sequence = state.controller.editor().revision().command_sequence;
                    if result.page_id != state.controller.page_id()
                        || result.revision_id != state.controller.revision_id()
                        || result.project_hash != current_hash.hex()
                        || result.command_sequence != current_sequence
                    {
                        state.fail(
                            "Browser workspace response no longer matches the active document",
                        );
                        return;
                    }
                    if current_hash != previous_hash
                        || current_active_page_index != previous_active_page_index
                    {
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
                                    state.fail(
                                        "Browser workspace request has a stale command sequence",
                                    );
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
                                    command_sequence,
                                    preview_attempt_id: None,
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
            save_outcome_unknown,
        );
    });
    #[cfg(feature = "dioxus-web")]
    let canvas_publish_confirmed = on_publish_confirmed.clone();
    #[cfg(feature = "dioxus-web")]
    let on_canvas_ui_intent = EventHandler::new(move |intent: UiIntent| {
        let write_conflict = *peer_write_conflict.peek()
            || save_outcome_unknown.peek().is_some()
            || matches!(
                &*recovery_status.peek(),
                DraftRecoveryStatus::Conflict { .. }
            )
            || session.read().controller.ui().state.dirty.save_conflict;
        if write_conflict && peer_conflict_blocks_ui_intent(&intent) {
            if matches!(&intent, UiIntent::Drop) {
                let cancel_effects = session
                    .write()
                    .dispatch(EditorSessionAction::DispatchIntent(UiIntent::CancelDrag));
                dispatch_facade_effects(
                    session,
                    canvas_facade.clone(),
                    cancel_effects,
                    canvas_publish_confirmed.clone(),
                    save_outcome_unknown,
                );
            }
            session
                .write()
                .fail("This editor is read-only until the save result is reconciled.");
            return;
        }
        if peer_conflict_blocks_ui_intent(&intent) {
            recovery_persisted.set(false);
        }
        let is_drop = matches!(&intent, UiIntent::Drop);
        let effects = session
            .write()
            .dispatch(EditorSessionAction::DispatchIntent(intent));
        dispatch_facade_effects(
            session,
            canvas_facade.clone(),
            effects,
            canvas_publish_confirmed.clone(),
            save_outcome_unknown,
        );
        if is_drop && session.read().controller.ui().state.drag.is_some() {
            // Fly Core intentionally keeps a rejected drop retryable. The browser drag source
            // has completed, so retire that drag session after the failed drop.
            let cancel_effects = session
                .write()
                .dispatch(EditorSessionAction::DispatchIntent(UiIntent::CancelDrag));
            dispatch_facade_effects(
                session,
                canvas_facade.clone(),
                cancel_effects,
                canvas_publish_confirmed.clone(),
                save_outcome_unknown,
            );
        }
    });

    let discard_store = recovery_store.clone();
    let mut discard_status = recovery_status;
    let mut discard_json = recovery_conflict_json;
    let mut discard_persisted = recovery_persisted;
    let on_discard_recovery = EventHandler::new(move |_| {
        if let Some(store) = discard_store.as_ref() {
            match store.clear_all() {
                Ok(()) => {
                    discard_status.set(DraftRecoveryStatus::Ready);
                    discard_json.set(None);
                    discard_persisted.set(true);
                }
                Err(error) => discard_status.set(DraftRecoveryStatus::Error(error)),
            }
        }
    });

    let retry_facade = props.facade.clone();
    let retry_publish_confirmed = on_publish_confirmed.clone();
    let on_retry_unknown_save = EventHandler::new(move |_| {
        if save_retry_in_progress
            || *peer_write_conflict.peek()
            || matches!(
                &*recovery_status.peek(),
                DraftRecoveryStatus::Conflict { .. }
            )
        {
            return;
        }
        let matches_submitted = save_outcome_unknown
            .peek()
            .as_ref()
            .is_some_and(|submitted| can_retry_unknown_save(&session.read().controller, submitted));
        if !matches_submitted {
            session.write().fail(
                "The working draft differs from the unknown save; preserve it and reload before saving.",
            );
            return;
        }
        let effects = session
            .write()
            .dispatch(EditorSessionAction::DispatchIntent(UiIntent::RequestSave));
        dispatch_facade_effects(
            session,
            retry_facade.clone(),
            effects,
            retry_publish_confirmed.clone(),
            save_outcome_unknown,
        );
    });

    #[cfg(feature = "dioxus-web")]
    let (workspace_canvas, workspace_canvas_interactive): (Option<Element>, bool) = {
        let mut canvas_document = workspace_document.clone();
        let active_page = canvas_document
            .project()
            .pages
            .get(workspace_initial_page)
            .cloned();
        match active_page {
            Some(active_page) => {
                canvas_document.project_mut().pages = vec![active_page];
                let canvas_nonce = workspace_csp_nonce.as_deref().and_then(UiCspNonce::new);
                let srcdoc = render_canvas_srcdoc_with_context(
                    &canvas_document,
                    &workspace_canvas_instance_id,
                    &workspace_runtime_context,
                    canvas_nonce.as_ref().map(UiCspNonce::as_str),
                );
                (
                    Some(rsx! {
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
                    true,
                )
            }
            None => (
                Some(rsx! {
                    div {
                        class: "rounded-md border border-destructive/40 bg-destructive/10 p-4 text-sm",
                        role: "alert",
                        "The active page is unavailable for isolated canvas rendering."
                    }
                }),
                false,
            ),
        }
    };
    #[cfg(not(feature = "dioxus-web"))]
    let (workspace_canvas, workspace_canvas_interactive): (Option<Element>, bool) = (None, false);

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
                            if *peer_write_conflict.peek()
                                || save_outcome_unknown.peek().is_some()
                                || recovery_conflict_active
                                || session.read().controller.ui().state.dirty.save_conflict
                            {
                                return;
                            }
                            recovery_persisted.set(false);
                            session.write().dispatch(EditorSessionAction::DispatchIntent(UiIntent::Undo));
                        },
                        "Undo"
                    }
                    button {
                        class: "px-3 py-1.5 text-sm rounded bg-muted hover:bg-muted/80 disabled:opacity-50",
                        disabled: !can_redo,
                        onclick: move |_| {
                            if *peer_write_conflict.peek()
                                || save_outcome_unknown.peek().is_some()
                                || recovery_conflict_active
                                || session.read().controller.ui().state.dirty.save_conflict
                            {
                                return;
                            }
                            recovery_persisted.set(false);
                            session.write().dispatch(EditorSessionAction::DispatchIntent(UiIntent::Redo));
                        },
                        "Redo"
                    }
                    button {
                        class: "px-3 py-1.5 text-sm rounded bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50",
                        disabled: !can_save,
                        onclick: move |_| {
                            if *peer_write_conflict.peek()
                                || save_outcome_unknown.peek().is_some()
                                || recovery_conflict_active
                                || session.read().controller.ui().state.dirty.save_conflict
                            {
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
                                save_outcome_unknown,
                            );
                        },
                        "Save"
                    }
                }
            }


            if let DraftRecoveryStatus::Restored { draft_count } = recovery_status_value.clone() {
                div {
                    class: "p-3 bg-sky-500/10 text-sky-900 dark:text-sky-200 text-sm border-b border-sky-500/30",
                    role: "status",
                    "Recovered {draft_count} local draft. Review the document and save it to persist the recovery."
                }
            }
            if let DraftRecoveryStatus::Merged { draft_count } = recovery_status_value.clone() {
                div {
                    class: "p-3 bg-sky-500/10 text-sky-900 dark:text-sky-200 text-sm border-b border-sky-500/30",
                    role: "status",
                    "Merged {draft_count} local drafts with the latest server revision. Review the result and save it to persist the merge."
                }
            }
            if let DraftRecoveryStatus::Disabled(reason) = recovery_status_value.clone() {
                if !reason.contains("Dioxus web build") {
                    div {
                        class: "p-3 bg-amber-500/10 text-amber-900 dark:text-amber-200 text-sm border-b border-amber-500/30",
                        role: "status",
                        "Local draft recovery is disabled: {reason}"
                        if !local_recovery_json_text.is_empty() {
                            details {
                                summary { class: "cursor-pointer font-medium", "Show current draft JSON to copy" }
                                pre {
                                    class: "mt-2 max-h-96 overflow-auto whitespace-pre-wrap break-all rounded border border-amber-500/30 bg-background p-2 text-xs text-foreground",
                                    tabindex: "0",
                                    aria_label: "Current local Page Builder draft JSON",
                                    "{local_recovery_json_text}"
                                }
                            }
                        }
                    }
                }
            }
            if let DraftRecoveryStatus::Error(message)
            | DraftRecoveryStatus::SaveError(message) = recovery_status_value.clone()
            {
                div {
                    class: "p-3 bg-amber-500/10 text-amber-900 dark:text-amber-200 text-sm border-b border-amber-500/30 space-y-2",
                    role: "alert",
                    "Local draft recovery could not complete: {message}. Copy the draft JSON before reloading."
                    if !recovery_conflict_json_text.is_empty() {
                        details {
                            summary { class: "cursor-pointer font-medium", "Show preserved recovery draft JSON" }
                            pre {
                                class: "mt-2 max-h-96 overflow-auto whitespace-pre-wrap break-all rounded border border-amber-500/30 bg-background p-2 text-xs text-foreground",
                                tabindex: "0",
                                aria_label: "Preserved local Page Builder recovery JSON",
                                "{recovery_conflict_json_text}"
                            }
                        }
                    } else if !local_recovery_json_text.is_empty() {
                        details {
                            summary { class: "cursor-pointer font-medium", "Show current editor document JSON" }
                            pre {
                                class: "mt-2 max-h-96 overflow-auto whitespace-pre-wrap break-all rounded border border-amber-500/30 bg-background p-2 text-xs text-foreground",
                                tabindex: "0",
                                aria_label: "Current Page Builder editor document JSON",
                                "{local_recovery_json_text}"
                            }
                        }
                        p {
                            class: "text-xs opacity-80",
                            "This is the current editor document and may include unsaved edits; an unreadable browser recovery record, if present, remains untouched."
                        }
                    }
                    if recovery_store.is_some() {
                        button {
                            class: "rounded border border-amber-600/40 px-3 py-1.5 font-medium hover:bg-amber-500/10",
                            onclick: move |_| on_discard_recovery.call(()),
                            "Discard unreadable local recovery records"
                        }
                    }
                }
            }
            if let DraftRecoveryStatus::Conflict { paths } = recovery_status_value.clone() {
                div {
                    class: "p-3 bg-destructive/10 text-destructive text-sm border-b border-destructive/30 space-y-2",
                    role: "alert",
                    p {
                        "The local draft could not be safely reconciled with the current server document. Nothing was overwritten. Editing and saving are paused until you preserve the local copy or explicitly discard it."
                    }
                    ul {
                        class: "list-disc pl-5 text-xs space-y-1",
                        for path in paths {
                            li { "{path}" }
                        }
                    }
                    if !recovery_conflict_json_text.is_empty() {
                        details {
                            summary { class: "cursor-pointer font-medium", "Show preserved local draft JSON" }
                            pre {
                                class: "mt-2 max-h-96 overflow-auto whitespace-pre-wrap break-all rounded border border-destructive/30 bg-background p-2 text-xs text-foreground",
                                tabindex: "0",
                                aria_label: "Preserved local recovery draft JSON",
                                "{recovery_conflict_json_text}"
                            }
                        }
                    }
                    button {
                        class: "rounded border border-destructive/40 px-3 py-1.5 font-medium hover:bg-destructive/10",
                        onclick: move |_| on_discard_recovery.call(()),
                        "Discard local copies and continue with server version"
                    }
                    p {
                        class: "text-xs opacity-80",
                        "This removes local recovery records for this page in this browser. Copy the JSON above first if you need to keep it."
                    }
                }
            }

            if peer_conflict_active && !recovery_conflict_active {
                div {
                    class: "p-3 bg-amber-500/10 text-amber-900 dark:text-amber-200 text-sm border-b border-amber-500/30 space-y-2",
                    role: "alert",
                    if *recovery_persisted.read() {
                        p {
                            "The server has a newer revision. This editor is read-only. Your local draft is saved in browser recovery storage; reload to merge it with the latest server version."
                        }
                    } else {
                        p {
                            "The server has a newer revision. This editor is read-only. Browser recovery storage is not confirmed, so copy your local draft before reloading."
                        }
                    }
                    details {
                        summary { class: "cursor-pointer font-medium", "Show local draft JSON to copy" }
                        pre {
                            class: "mt-2 max-h-96 overflow-auto whitespace-pre-wrap break-all rounded border border-amber-500/30 bg-background p-2 text-xs text-foreground",
                            tabindex: "0",
                            aria_label: "Local Page Builder draft JSON",
                            "{local_recovery_json_text}"
                        }
                    }
                    if let Some(reload) = props.on_reload_requested {
                        button {
                            class: "rounded border border-amber-600/40 px-3 py-1.5 font-medium hover:bg-amber-500/10 disabled:opacity-50",
                            disabled: copied_current_draft,
                            onclick: move |_| local_draft_copied.set(Some(displayed_draft_stamp)),
                            "I copied the local draft"
                        }
                        button {
                            class: "rounded border border-amber-600/40 px-3 py-1.5 font-medium hover:bg-amber-500/10 disabled:opacity-50",
                            disabled: !can_reload_draft,
                            onclick: move |_| {
                                if draft_reload_safe(
                                    &session.read().controller,
                                    *recovery_persisted.peek(),
                                    *local_draft_copied.peek(),
                                ) {
                                    reload.call(());
                                }
                            },
                            "Reload latest revision"
                        }
                    } else {
                        if *recovery_persisted.read() {
                            p { "The local draft is stored in browser recovery storage. Reload this editor using the host's page refresh action." }
                        } else {
                            p { "After copying the draft, reload this editor using the host's page refresh action." }
                        }
                    }
                }
            } else if save_outcome_unknown_active && !recovery_conflict_active {
                div {
                    class: "p-3 bg-amber-500/10 text-amber-900 dark:text-amber-200 text-sm border-b border-amber-500/30 space-y-2",
                    role: "alert",
                    p {
                        "The save outcome is not yet known. Do not edit this draft or start a different save."
                    }
                    if unknown_save_retry_matches {
                        p { "Retrying the exact same request is safe: Pages will replay the committed receipt if the first request succeeded." }
                    } else {
                        p { "The working draft differs from the submitted save. Preserve your draft and reload the latest server version to reconcile it. A different payload cannot be retried as the original request." }
                    }
                    if let Some(err) = last_error.clone() {
                        p { class: "text-xs opacity-80", "{err}" }
                    }
                    details {
                        summary { class: "cursor-pointer font-medium", "Show local draft JSON to copy" }
                        pre {
                            class: "mt-2 max-h-96 overflow-auto whitespace-pre-wrap break-all rounded border border-amber-500/30 bg-background p-2 text-xs text-foreground",
                            tabindex: "0",
                            aria_label: "Local Page Builder draft JSON",
                            "{local_recovery_json_text}"
                        }
                    }
                    button {
                        class: "rounded border border-amber-600/40 px-3 py-1.5 font-medium hover:bg-amber-500/10 disabled:opacity-50",
                        disabled: save_retry_in_progress || !unknown_save_retry_matches || props.facade.is_none(),
                        onclick: move |_| on_retry_unknown_save.call(()),
                        "Retry exact same save"
                    }
                    if let Some(reload) = props.on_reload_requested {
                        if !*recovery_persisted.read() {
                            p { "Browser recovery storage is not confirmed. Copy the current draft JSON before reloading." }
                        }
                        button {
                            class: "rounded border border-amber-600/40 px-3 py-1.5 font-medium hover:bg-amber-500/10 disabled:opacity-50",
                            disabled: copied_current_draft,
                            onclick: move |_| local_draft_copied.set(Some(displayed_draft_stamp)),
                            "I copied the local draft"
                        }
                        button {
                            class: "rounded border border-amber-600/40 px-3 py-1.5 font-medium hover:bg-amber-500/10 disabled:opacity-50",
                            disabled: !can_reload_draft,
                            onclick: move |_| {
                                if draft_reload_safe(
                                    &session.read().controller,
                                    *recovery_persisted.peek(),
                                    *local_draft_copied.peek(),
                                ) {
                                    reload.call(());
                                }
                            },
                            "Reload latest server version"
                        }
                    }
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
                            canvas_interactive: Some(workspace_canvas_interactive),
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
    fn reload_requires_persisted_recovery_or_copy_of_current_revision() {
        let mut controller = AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "component": { "id": "root", "type": "wrapper", "components": [{
                        "id": "hero", "type": "text", "content": "original"
                    }] }
                }]
            }),
        )
        .expect("controller");
        let copied = Some(draft_stamp(&controller));
        assert!(!draft_reload_safe(&controller, false, None));
        assert!(draft_reload_safe(&controller, true, None));
        assert!(draft_reload_safe(&controller, false, copied));

        controller
            .dispatch(UiIntent::execute(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch::default().set_field("content", json!("new unsaved content")),
            }))
            .expect("edit");
        assert!(!draft_reload_safe(&controller, false, copied));
        assert!(draft_reload_safe(&controller, true, copied));
        assert!(draft_reload_safe(
            &controller,
            false,
            Some(draft_stamp(&controller))
        ));
        controller.dispatch(UiIntent::Undo).expect("undo");
        // Even returning to the same content does not reuse a prior confirmation.
        assert!(!draft_reload_safe(&controller, false, copied));
    }

    #[test]
    fn unknown_save_retry_requires_exact_submitted_payload_and_revision() {
        let mut controller = AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "component": { "id": "root", "type": "wrapper", "components": [{
                        "id": "hero", "type": "text", "content": "submitted"
                    }] }
                }]
            }),
        )
        .expect("controller");
        let input = PublishPageBuilderInput {
            page_id: controller.page_id().to_string(),
            revision_id: controller.revision_id().to_string(),
            project_data: GrapesJsCodec::encode_value(controller.editor().document())
                .expect("encode"),
        };
        assert!(can_retry_unknown_save(&controller, &input));
        assert!(same_publish_input(&input, &input));
        let mut different = input.clone();
        different.revision_id = "rev-2".to_string();
        assert!(!can_retry_unknown_save(&controller, &different));
        assert!(!same_publish_input(&input, &different));
        different = input.clone();
        different.page_id = "another".to_string();
        assert!(!can_retry_unknown_save(&controller, &different));
        assert!(!same_publish_input(&input, &different));

        controller
            .dispatch(UiIntent::execute(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch::default().set_field("content", json!("newer unsaved edit")),
            }))
            .expect("edit while saving");
        assert!(!can_retry_unknown_save(&controller, &input));
        different = input.clone();
        different.project_data =
            GrapesJsCodec::encode_value(controller.editor().document()).expect("encode");
        assert!(!same_publish_input(&input, &different));
        controller
            .dispatch(UiIntent::Undo)
            .expect("restore original payload");
        assert!(can_retry_unknown_save(&controller, &input));
    }

    #[test]
    fn props_detect_draft_changes_without_a_new_server_revision() {
        let controller = AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "component": { "id": "root", "type": "wrapper", "components": [{
                        "id": "hero", "type": "text", "content": "one"
                    }] }
                }]
            }),
        )
        .expect("controller");
        let original = DioxusPageBuilderAdminHostContext::new(controller);
        let mut changed = original.clone();
        changed
            .controller
            .dispatch(UiIntent::execute(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch::default().set_field("content", json!("two")),
            }))
            .expect("edit");
        assert_eq!(
            original.controller.revision_id(),
            changed.controller.revision_id()
        );
        assert!(original != changed, "draft edit must trigger prop update");
        let mut workspace = original.clone();
        workspace.adapter_workspace = false;
        assert!(original != workspace, "mount mode must trigger prop update");

        let mut recovery_scope = original.clone();
        recovery_scope.draft_recovery_scope = Some("tenant-actor-scope".to_string());
        assert!(
            original != recovery_scope,
            "recovery scope changes must remount the isolated local recovery store"
        );
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
        )
        .expect("controller");
        controller
            .dispatch(UiIntent::SetEditableCapabilities(
                CapabilityState::read_only(),
            ))
            .expect("apply the host restriction");
        assert!(!controller.ui().state.capabilities.edit);
        assert!(!controller.ui().state.capabilities.history);
        assert!(controller.dispatch(UiIntent::RequestSave).is_err());
        assert!(controller.dispatch(UiIntent::Undo).is_err());
    }

    #[test]
    fn local_recovery_json_contains_the_current_unsaved_document() {
        let controller = AdminCanvasController::new(
            "home",
            "rev-1",
            json!({ "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "hero",
                        "type": "text",
                        "content": "unsaved local work"
                    }]
                }
            }] }),
        )
        .expect("controller");
        let recovered = local_recovery_json(&controller);
        assert!(recovered.contains("unsaved local work"));
        let decoded: Value = serde_json::from_str(&recovered).expect("recovery JSON");
        let expected =
            GrapesJsCodec::encode_value(controller.editor().document()).expect("GrapesJS project");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn workspace_intents_are_bound_to_the_rendered_page_revision_and_hash() {
        let document = json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "hero", "type": "text", "content": "one" }]
                }
            }]
        });
        let rendered = AdminCanvasController::new("home", "rev-1", document.clone())
            .expect("rendered controller");
        let rendered_hash = rendered.editor().revision().project_hash.hex();
        let envelope = bind_workspace_intent_snapshot(
            BrowserIntentEnvelope::new(
                "home",
                fly_browser::BrowserIntentKind::RemoveSelected,
                serde_json::json!({}),
            ),
            "home",
            "rev-1",
            &rendered_hash,
            rendered.editor().revision().command_sequence,
        );

        let mut newer_revision =
            AdminCanvasController::new("home", "rev-2", document.clone()).expect("newer revision");
        assert!(matches!(
            crate::dispatch_browser_intent(&mut newer_revision, envelope.clone()),
            Err(crate::BrowserIntentDispatchError::RevisionConflict { .. })
                | Err(crate::BrowserIntentDispatchError::CommandSequenceConflict { .. })
        ));

        let changed_document = json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "hero", "type": "text", "content": "two" }]
                }
            }]
        });
        let mut changed_snapshot = AdminCanvasController::new("home", "rev-1", changed_document)
            .expect("changed snapshot");
        assert!(matches!(
            crate::dispatch_browser_intent(&mut changed_snapshot, envelope.clone()),
            Err(crate::BrowserIntentDispatchError::ProjectHashConflict { .. })
        ));

        let mut other_page =
            AdminCanvasController::new("about", "rev-1", document).expect("other page");
        assert!(matches!(
            crate::dispatch_browser_intent(&mut other_page, envelope),
            Err(crate::BrowserIntentDispatchError::PageScopeConflict { .. })
        ));
    }

    #[test]
    fn server_revision_conflict_uses_the_same_local_recovery_banner_as_peer_conflict() {
        let mut controller = AdminCanvasController::new(
            "home",
            "rev-1",
            json!({ "pages": [{
                "id": "home",
                "component": { "id": "root", "type": "wrapper" }
            }] }),
        )
        .expect("controller");
        controller
            .mark_save_conflict("rev-1".to_string(), "rev-2".to_string())
            .expect("mark server conflict");
        assert!(write_conflict_active(false, &controller));
        assert!(write_conflict_active(true, &controller));
    }

    #[test]
    fn peer_conflict_uses_local_recovery_banner_without_server_conflict_state() {
        let controller = AdminCanvasController::new(
            "home",
            "rev-1",
            json!({ "pages": [{
                "id": "home",
                "component": { "id": "root", "type": "wrapper" }
            }] }),
        )
        .expect("controller");
        assert!(!write_conflict_active(false, &controller));
        assert!(write_conflict_active(true, &controller));
    }

    #[test]
    fn canvas_iframe_identity_tracks_document_revision_and_hash() {
        let first = canvas_instance_id("home", 0, "rev-1", "hash-1", 7);
        assert_ne!(first, canvas_instance_id("home", 1, "rev-1", "hash-1", 7));
        assert_ne!(first, canvas_instance_id("home", 0, "rev-1", "hash-2", 7));
        assert_ne!(first, canvas_instance_id("home", 0, "rev-2", "hash-1", 7));
        assert_ne!(first, canvas_instance_id("about", 0, "rev-1", "hash-1", 7));
        assert_ne!(first, canvas_instance_id("home", 0, "rev-1", "hash-1", 8));
        assert!(first.starts_with("fly-canvas-"));
    }

    #[test]
    fn peer_write_fence_blocks_mutations_but_allows_selection_and_cancel() {
        assert!(peer_conflict_blocks_ui_intent(&UiIntent::Drop));
        assert!(peer_conflict_blocks_ui_intent(&UiIntent::Undo));
        assert!(!peer_conflict_blocks_ui_intent(&UiIntent::Select(Some(
            "hero".to_string()
        ))));
        assert!(!peer_conflict_blocks_ui_intent(&UiIntent::CancelDrag));
    }

    #[test]
    fn session_keys_are_scoped_to_page_and_revision() {
        let doc = json!({
            "pages": [{ "id": "home", "component": { "id": "root", "type": "wrapper" } }]
        });
        let home_v1 = AdminCanvasController::new("home", "rev-1", doc.clone()).expect("first page");
        let home_v2 =
            AdminCanvasController::new("home", "rev-2", doc.clone()).expect("second revision");
        let other = AdminCanvasController::new("other", "rev-1", doc).expect("second page");
        assert_ne!(
            controller_session_key(&home_v1),
            controller_session_key(&home_v2)
        );
        assert_ne!(
            controller_session_key(&home_v1),
            controller_session_key(&other)
        );

        let mut changed_draft = home_v1.clone();
        changed_draft
            .dispatch(UiIntent::execute(EditorCommand::Patch {
                component_id: "root".to_string(),
                patch: ComponentPatch::default().set_attribute(
                    "aria-label",
                    json!("Updated draft without a new server revision"),
                ),
            }))
            .expect("patch existing root");
        assert_eq!(changed_draft.revision_id(), home_v1.revision_id());
        assert_ne!(
            controller_session_key(&home_v1),
            controller_session_key(&changed_draft)
        );

        let mut selected = home_v1.clone();
        selected
            .dispatch(UiIntent::Select(Some("root".to_string())))
            .expect("select root");
        assert_ne!(
            controller_session_key(&home_v1),
            controller_session_key(&selected)
        );
    }
}
