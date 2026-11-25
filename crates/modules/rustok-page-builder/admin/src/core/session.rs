use crate::model::{AdminCanvasController, AdminCanvasEffect, SaveAttempt};
use fly::{GrapesJsCodec, ProjectHash, TraitSchemaRegistry, ValidationSeverity};
use fly_ui::{CapabilityState, EditorCapability, EditorCapabilityEvaluation, UiIntent};
use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PageBuilderPreviewRuntime, PreviewPageBuilderInput,
    PreviewPageBuilderResult,
};
use rustok_page_builder::evaluate_host_runtime_publish_gate;
use rustok_page_builder::{
    RuntimeContextScenario, RuntimePublishGateEvaluation, RuntimePublishGatePolicy,
};
use serde_json::{Map, Value};
use std::sync::Arc;

fn optional_arc_changed<T>(current: &Option<Arc<T>>, next: &Option<Arc<T>>) -> bool {
    match (current, next) {
        (None, None) => false,
        (Some(current), Some(next)) => !Arc::ptr_eq(current, next),
        _ => true,
    }
}

/// Framework-agnostic mutable state of an editor session.
///
/// Contains zero framework-specific reactive primitives (no Leptos Signals, no Dioxus Signals).
/// Can be managed by any reactive wrapper or reducer.
fn conflict_actual_revision(message: &str) -> Option<String> {
    for marker in ["found `", "current revision `"] {
        if let Some((_, tail)) = message.split_once(marker) {
            let revision = tail.split('`').next()?.trim();
            if !revision.is_empty() {
                return Some(revision.to_string());
            }
        }
    }
    None
}

/// Local correlation for a single asynchronous server preview. The snapshot
/// describes the inputs, while the UUID distinguishes identical retries.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewAttempt {
    pub(crate) id: uuid::Uuid,
    pub(crate) snapshot: (ProjectHash, usize, Value, Option<String>),
}

impl PreviewAttempt {
    pub(crate) fn new(
        hash: ProjectHash,
        page_index: usize,
        context: Value,
        scenario: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            snapshot: (hash, page_index, context, scenario),
        }
    }

    pub fn id(&self) -> uuid::Uuid {
        self.id
    }
}

#[derive(Clone)]
pub struct EditorSessionState {
    pub controller: AdminCanvasController,
    pub last_error: Option<String>,
    pub last_announcement: Option<String>,
    pub server_preview_html: Option<String>,
    pub preview_in_progress: bool,
    pub trait_schemas: Arc<TraitSchemaRegistry>,
    pub editor_capability_evaluation: Option<Arc<EditorCapabilityEvaluation>>,
    pub runtime_context: Value,
    pub runtime_context_configured: bool,
    pub runtime_scenarios: Arc<Vec<RuntimeContextScenario>>,
    pub active_runtime_scenario: Option<String>,
    pub runtime_publish_gate_policy: Option<Arc<RuntimePublishGatePolicy>>,
    pub runtime_publish_gate_evaluation: Option<RuntimePublishGateEvaluation>,
    pub preview_request: Option<PreviewAttempt>,
    pub facade_missing: String,
    pub save_succeeded: String,
}

impl EditorSessionState {
    pub fn new(
        controller: AdminCanvasController,
        facade_missing: impl Into<String>,
        save_succeeded: impl Into<String>,
    ) -> Self {
        Self {
            controller,
            last_error: None,
            last_announcement: None,
            server_preview_html: None,
            preview_in_progress: false,
            trait_schemas: Arc::new(TraitSchemaRegistry::with_builtins()),
            editor_capability_evaluation: None,
            runtime_context: Value::Object(Map::new()),
            runtime_context_configured: false,
            runtime_scenarios: Arc::new(Vec::new()),
            active_runtime_scenario: None,
            runtime_publish_gate_policy: None,
            runtime_publish_gate_evaluation: None,
            preview_request: None,
            facade_missing: facade_missing.into(),
            save_succeeded: save_succeeded.into(),
        }
    }

    pub fn with_trait_schemas(mut self, trait_schemas: Arc<TraitSchemaRegistry>) -> Self {
        self.trait_schemas = trait_schemas;
        self
    }

    pub fn with_editor_capability_evaluation(
        mut self,
        evaluation: Arc<EditorCapabilityEvaluation>,
    ) -> Self {
        self.editor_capability_evaluation = Some(evaluation);
        self
    }

    pub fn capability_enabled(&self, capability: EditorCapability) -> bool {
        self.controller.ui().state.capabilities.allows(capability)
    }

    pub fn with_runtime_context(mut self, runtime_context: Value) -> Self {
        self.runtime_context = runtime_context;
        self.runtime_context_configured = true;
        self.active_runtime_scenario = None;
        self.runtime_publish_gate_evaluation = None;
        self
    }

    pub fn with_runtime_scenarios(
        mut self,
        runtime_scenarios: Arc<Vec<RuntimeContextScenario>>,
    ) -> Self {
        self.runtime_scenarios = runtime_scenarios;
        self.runtime_publish_gate_evaluation = None;
        self
    }

    pub fn with_runtime_publish_gate_policy(
        mut self,
        policy: Arc<RuntimePublishGatePolicy>,
    ) -> Self {
        self.runtime_publish_gate_policy = Some(policy);
        self.runtime_publish_gate_evaluation = None;
        self
    }

    /// Reconciles host-owned configuration when a long-lived UI component receives new props.
    ///
    /// Dioxus preserves hook state across prop rerenders, so constructor-only configuration
    /// silently becomes stale unless the host explicitly refreshes the session.
    pub fn update_host_configuration(
        &mut self,
        trait_schemas: Option<Arc<TraitSchemaRegistry>>,
        editor_capability_evaluation: Option<Arc<EditorCapabilityEvaluation>>,
        runtime_context: Option<Value>,
        runtime_scenarios: Option<Arc<Vec<RuntimeContextScenario>>>,
        runtime_publish_gate_policy: Option<Arc<RuntimePublishGatePolicy>>,
        facade_available: bool,
    ) {
        let runtime_context_configured = runtime_context.is_some();
        let runtime_context = runtime_context.unwrap_or_else(|| Value::Object(Map::new()));
        let trait_schemas = trait_schemas
            .unwrap_or_else(|| Arc::new(TraitSchemaRegistry::with_builtins()));
        let trait_schemas_changed = self.trait_schemas != trait_schemas;
        let (runtime_scenarios_changed, runtime_scenarios) = match runtime_scenarios {
            Some(next) => (self.runtime_scenarios != next, next),
            None => (
                !self.runtime_scenarios.is_empty(),
                Arc::new(Vec::new()),
            ),
        };
        let runtime_inputs_changed = self.runtime_context != runtime_context
            || self.runtime_context_configured != runtime_context_configured
            || trait_schemas_changed
            || optional_arc_changed(
                &self.editor_capability_evaluation,
                &editor_capability_evaluation,
            )
            || runtime_scenarios_changed
            || optional_arc_changed(
                &self.runtime_publish_gate_policy,
                &runtime_publish_gate_policy,
            );

        self.trait_schemas = trait_schemas;
        self.editor_capability_evaluation = editor_capability_evaluation;
        self.runtime_context = runtime_context;
        self.runtime_context_configured = runtime_context_configured;
        self.runtime_scenarios = runtime_scenarios;
        self.runtime_publish_gate_policy = runtime_publish_gate_policy;

        if runtime_inputs_changed {
            self.active_runtime_scenario = None;
            self.runtime_publish_gate_evaluation = None;
            self.server_preview_html = None;
            self.preview_request = None;
            self.preview_in_progress = false;
        }
        if facade_available && self.last_error.as_deref() == Some(self.facade_missing.as_str()) {
            self.last_error = None;
        }
    }

    pub fn apply_runtime_scenario(&mut self, scenario_id: &str) -> bool {
        let Some(scenario) = self
            .runtime_scenarios
            .iter()
            .find(|scenario| scenario.id == scenario_id)
        else {
            self.fail(format!(
                "Runtime context scenario `{scenario_id}` was not found"
            ));
            return false;
        };
        self.runtime_context = scenario.context.clone();
        self.runtime_context_configured = true;
        self.active_runtime_scenario = Some(scenario.id.clone());
        self.runtime_publish_gate_evaluation = None;
        self.server_preview_html = None;
        self.preview_request = None;
        self.preview_in_progress = false;
        self.last_error = None;
        self.announce(format!("Preview scenario applied: {}", scenario.label));
        true
    }

    pub fn set_runtime_context(&mut self, runtime_context: Value) {
        self.runtime_context = runtime_context;
        self.runtime_context_configured = true;
        self.active_runtime_scenario = None;
        self.runtime_publish_gate_evaluation = None;
        self.server_preview_html = None;
        self.preview_request = None;
        self.preview_in_progress = false;
    }

    pub fn evaluate_runtime_publish_gate(&self) -> Option<RuntimePublishGateEvaluation> {
        let policy = self.runtime_publish_gate_policy.as_ref()?;
        let configured = self.runtime_context_configured;
        Some(evaluate_host_runtime_publish_gate(
            self.controller.editor().document(),
            configured.then_some(&self.runtime_context),
            self.runtime_scenarios.as_slice(),
            policy.as_ref(),
        ))
    }

    pub fn prepare_server_preview_request(
        &mut self,
    ) -> Result<
        (
            PageBuilderCapabilityRequest,
            ProjectHash,
            usize,
            Value,
            Option<String>,
        ),
        String,
    > {
        if self.preview_in_progress {
            return Err("Preview already in progress".to_string());
        }
        let runtime_context = if self.runtime_context_configured {
            self.runtime_context.clone()
        } else {
            Value::Object(Map::new())
        };
        let runtime_scenario_id = self.active_runtime_scenario.clone();
        let active_page_index = self.controller.active_page_index();
        let mut document = self.controller.editor().document().clone();
        let active_page = document
            .project()
            .pages
            .get(active_page_index)
            .cloned()
            .into_iter()
            .collect();
        document.project_mut().pages = active_page;
        let project_data = GrapesJsCodec::encode_value(&document).map_err(|e| e.to_string())?;
        let project_hash = self.controller.editor().revision().project_hash;
        let request = PageBuilderCapabilityRequest::Preview(
            PreviewPageBuilderInput::new(self.controller.page_id(), project_data).with_runtime(
                PageBuilderPreviewRuntime::new(
                    runtime_context.clone(),
                    runtime_scenario_id.clone(),
                ),
            ),
        );
        self.preview_request = Some(PreviewAttempt::new(
            project_hash,
            active_page_index,
            runtime_context.clone(),
            runtime_scenario_id.clone(),
        ));
        self.preview_in_progress = true;
        Ok((
            request,
            project_hash,
            active_page_index,
            runtime_context,
            runtime_scenario_id,
        ))
    }

    pub fn handle_preview_response(
        &mut self,
        attempt_id: uuid::Uuid,
        response: &PreviewPageBuilderResult,
    ) -> Result<(), String> {
        if !self.preview_attempt_active(attempt_id) {
            return Ok(());
        }
        let preview_request = self.preview_request.take();
        self.preview_in_progress = false;
        let current_runtime_context = if self.runtime_context_configured {
            self.runtime_context.clone()
        } else {
            Value::Object(Map::new())
        };
        let current_runtime_scenario = self.active_runtime_scenario.clone();
        let current_page_id = self.controller.page_id();
        let current_hash = self.controller.editor().revision().project_hash;
        let current_active_page = self.controller.active_page_index();

        if response.page_id != current_page_id {
            let msg = format!(
                "Page Builder preview returned page `{}` for `{}`",
                response.page_id, current_page_id
            );
            self.fail(&msg);
            return Err(msg);
        }
        if response.runtime_scenario_id != current_runtime_scenario {
            let msg =
                "Page Builder preview returned a different runtime scenario; refresh the preview"
                    .to_string();
            self.fail(&msg);
            return Err(msg);
        }
        if preview_request.is_none_or(|expected| {
            expected.snapshot
                != (
                    current_hash,
                    current_active_page,
                    current_runtime_context,
                    current_runtime_scenario,
                )
        }) {
            let msg = "Page Builder project or runtime context changed while the server preview was rendering; refresh the preview".to_string();
            self.fail(&msg);
            return Err(msg);
        }
        self.server_preview_html = Some(response.html.clone());
        self.last_error = None;
        self.announce("Server preview refreshed");
        Ok(())
    }

    pub(crate) fn preview_attempt_active(&self, attempt_id: uuid::Uuid) -> bool {
        self.preview_in_progress
            && self.preview_request.as_ref().is_some_and(|attempt| attempt.id == attempt_id)
    }

    pub(crate) fn handle_preview_failure(&mut self, attempt_id: uuid::Uuid, message: String) {
        if self.preview_attempt_active(attempt_id) {
            self.preview_in_progress = false;
            self.preview_request = None;
            self.fail(message);
        }
    }

    pub fn handle_publish_response(
        &mut self,
        page_id: &str,
        revision_id: &str,
        expected_hash: Option<ProjectHash>,
        attempt: &SaveAttempt,
    ) -> Result<(), String> {
        if self.controller.active_save_attempt().as_ref() != Some(attempt) {
            return Err("stale publish response belongs to an inactive save request".to_string());
        }
        if page_id != self.controller.page_id() {
            let msg = format!(
                "Page Builder facade returned page `{}` for `{}`",
                page_id,
                self.controller.page_id()
            );
            self.handle_publish_error_for_attempt(attempt, msg.clone());
            return Err(msg);
        }
        let Some(hash) = expected_hash else {
            let msg = "Publish acknowledgement has no submitted project hash".to_string();
            self.handle_publish_error_for_attempt(attempt, msg.clone());
            return Err(msg);
        };
        match self.controller.acknowledge_save_for_attempt(
            hash,
            attempt,
            revision_id.to_string(),
        ) {
            Ok(()) => {
                self.last_error = None;
                self.announce(self.save_succeeded.clone());
                Ok(())
            }
            Err(error) => {
                let msg = error.to_string();
                self.handle_publish_error_for_attempt(attempt, msg.clone());
                Err(msg)
            }
        }
    }

    /// Response-bound completion for framework adapters that own asynchronous facade requests.
    /// The request UUID prevents a delayed response from completing an identical retry after
    /// the controller has been replaced or restored.
    pub fn handle_publish_response_for_attempt(
        &mut self,
        page_id: &str,
        revision_id: &str,
        expected_hash: Option<ProjectHash>,
        attempt: &SaveAttempt,
    ) -> Result<(), String> {
        self.handle_publish_response(page_id, revision_id, expected_hash, attempt)
    }

    /// Returns false for a stale failure; the active newer request is untouched.
    pub fn handle_publish_error_for_attempt(
        &mut self,
        attempt: &SaveAttempt,
        message: impl Into<String>,
    ) -> bool {
        match self.controller.mark_save_failed_for_attempt(attempt) {
            Ok(true) => {
                self.fail(message);
                true
            }
            Ok(false) => false,
            Err(error) => {
                tracing::warn!(?error, "Failed to reject active publish attempt");
                false
            }
        }
    }

    /// A server revision conflict retires only the matching attempt and fences edits until
    /// the host reloads the newer persisted document.
    pub fn handle_publish_conflict_for_attempt(
        &mut self,
        attempt: &SaveAttempt,
        message: impl Into<String>,
    ) -> bool {
        let message = message.into();
        let expected = self.controller.revision_id().to_string();
        let actual = conflict_actual_revision(&message)
            .unwrap_or_else(|| "newer server revision".to_string());
        match self.controller.mark_save_conflict_for_attempt(
            attempt,
            expected,
            actual,
        ) {
            Ok(true) => {
                if let Err(error) = self
                    .controller
                    .dispatch(UiIntent::SetEditableCapabilities(CapabilityState::read_only()))
                {
                    tracing::warn!(?error, "Failed to fence after Page Builder revision conflict");
                }
                self.fail(message);
                true
            }
            Ok(false) => false,
            Err(error) => {
                tracing::warn!(?error, "Failed to mark Page Builder revision conflict");
                false
            }
        }
    }

    /// A sibling editor has committed a newer revision. Retire any in-flight local
    /// save attempt before its delayed response can acknowledge an obsolete snapshot.
    pub fn handle_peer_write_conflict(
        &mut self,
        actual_revision: impl Into<String>,
    ) -> bool {
        let actual = actual_revision.into();
        let expected = self.controller.revision_id().to_string();
        let message = format!(
            "Another tab saved this page at revision {actual}. Preserve your local changes and reload before editing again."
        );
        let marked = match self
            .controller
            .mark_peer_write_conflict(expected, actual)
        {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(?error, "Failed to record Page Builder peer-write conflict");
                false
            }
        };
        if let Err(error) = self.controller.dispatch(UiIntent::SetEditableCapabilities(
            fly_ui::CapabilityState::read_only(),
        )) {
            tracing::warn!(?error, "Failed to fence Page Builder after peer write");
        }
        self.fail(message);
        marked
    }

    pub fn dispatch_intent(&mut self, intent: UiIntent) -> Result<Vec<AdminCanvasEffect>, String> {
        if intent.mutates_document() || matches!(&intent, UiIntent::ActivatePage { .. }) {
            self.runtime_publish_gate_evaluation = None;
            self.server_preview_html = None;
            self.preview_request = None;
            self.preview_in_progress = false;
        }
        match self.controller.dispatch(intent) {
            Ok(effects) => {
                self.last_error = None;
                for effect in &effects {
                    if let AdminCanvasEffect::Announce(msg) = effect {
                        self.announce(msg.clone());
                    }
                }
                Ok(effects)
            }
            Err(e) => {
                let msg = e.to_string();
                self.fail(&msg);
                Err(msg)
            }
        }
    }

    pub fn mark_save_started(&mut self) -> Result<(), String> {
        self.mark_save_started_with_id().map(|_| ())
    }

    pub fn mark_save_started_with_id(&mut self) -> Result<SaveAttempt, String> {
        self.controller
            .mark_save_started_with_id()
            .map_err(|error| error.to_string())
    }

    pub fn mark_save_failed_for_attempt(
        &mut self,
        attempt: &SaveAttempt,
        message: impl Into<String>,
    ) -> bool {
        match self.controller.mark_save_failed_for_attempt(attempt) {
            Ok(true) => {
                self.fail(message);
                true
            }
            Ok(false) => false,
            Err(error) => {
                tracing::warn!(?error, "Failed to mark correlated save attempt failed");
                false
            }
        }
    }

    /// Uncorrelated failure notifications are only valid when no save is active.
    pub fn mark_save_failed(&mut self) {
        if let Err(error) = self.controller.mark_save_failed() {
            tracing::warn!(?error, "Failed to mark save failed in controller");
        }
    }

    pub fn announce(&mut self, message: impl Into<String>) {
        self.last_announcement = Some(message.into());
    }

    pub fn fail(&mut self, message: impl Into<String>) {
        self.last_error = Some(message.into());
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    pub fn dispatch(&mut self, action: EditorSessionAction) -> Vec<EditorSessionEffect> {
        let mut effects = Vec::new();
        match action {
            EditorSessionAction::DispatchIntent(intent) => {
                if let Ok(canvas_effects) = self.dispatch_intent(intent) {
                    for effect in canvas_effects {
                        match effect {
                            AdminCanvasEffect::Announce(msg) => {
                                effects.push(EditorSessionEffect::Announce(msg));
                            }
                            AdminCanvasEffect::Request {
                                request,
                                expected_hash,
                                ..
                            } => {
                                effects.push(EditorSessionEffect::ExecuteRequest {
                                    request,
                                    expected_hash,
                                    preview_attempt_id: None,
                                });
                            }
                        }
                    }
                }
            }
            EditorSessionAction::ApplyRuntimeScenario(scenario_id) => {
                self.apply_runtime_scenario(&scenario_id);
            }
            EditorSessionAction::SetRuntimeContext(ctx) => {
                self.set_runtime_context(ctx);
            }
            EditorSessionAction::RequestServerPreview => {
                match self.prepare_server_preview_request() {
                    Ok((request, hash, _, _, _)) => {
                        effects.push(EditorSessionEffect::ExecuteRequest {
                            request,
                            expected_hash: Some(hash),
                            preview_attempt_id: self.preview_request.as_ref().map(PreviewAttempt::id),
                        });
                    }
                    Err(e) => {
                        self.fail(e);
                    }
                }
            }
            EditorSessionAction::SaveStarted => match self.mark_save_started_with_id() {
                Ok(attempt) => effects.push(EditorSessionEffect::SaveAttemptStarted(attempt)),
                Err(error) => tracing::warn!(error = ?error, "Failed to mark save started"),
            },
            EditorSessionAction::SaveAcknowledged {
                page_id,
                revision_id,
                expected_hash,
                attempt,
            } => {
                if let Err(err) = self.handle_publish_response(
                    &page_id,
                    &revision_id,
                    expected_hash,
                    &attempt,
                ) {
                    // A peer-write fence already carries the actionable conflict message.
                    // Do not replace it with the less useful "inactive request" response.
                    if !self.controller.ui().state.dirty.save_conflict {
                        self.fail(err);
                    }
                }
            }
            EditorSessionAction::SaveFailed { attempt, message } => {
                self.mark_save_failed_for_attempt(&attempt, message);
            }
            EditorSessionAction::PreviewAcknowledged { attempt_id, response } => {
                if let Err(err) = self.handle_preview_response(attempt_id, &response) {
                    self.fail(err);
                }
            }
            EditorSessionAction::PreviewFailed { attempt_id, message } => {
                self.handle_preview_failure(attempt_id, message);
            }
            EditorSessionAction::Announce(msg) => {
                self.announce(msg);
            }
            EditorSessionAction::Fail(err) => {
                self.fail(err);
            }
            EditorSessionAction::ClearError => {
                self.clear_error();
            }
        }
        effects
    }
}

#[derive(Clone, Debug)]
pub enum EditorSessionAction {
    DispatchIntent(UiIntent),
    ApplyRuntimeScenario(String),
    SetRuntimeContext(Value),
    RequestServerPreview,
    SaveStarted,
    SaveAcknowledged {
        page_id: String,
        revision_id: String,
        expected_hash: Option<ProjectHash>,
        attempt: SaveAttempt,
    },
    SaveFailed {
        attempt: SaveAttempt,
        message: String,
    },
    PreviewAcknowledged {
        attempt_id: uuid::Uuid,
        response: Box<PreviewPageBuilderResult>,
    },
    PreviewFailed {
        attempt_id: uuid::Uuid,
        message: String,
    },
    Announce(String),
    Fail(String),
    ClearError,
}

#[derive(Clone, Debug)]
pub enum EditorSessionEffect {
    Announce(String),
    SaveAttemptStarted(SaveAttempt),
    ExecuteRequest {
        request: PageBuilderCapabilityRequest,
        expected_hash: Option<ProjectHash>,
        preview_attempt_id: Option<uuid::Uuid>,
    },
}

pub fn gate_error_message(evaluation: &RuntimePublishGateEvaluation) -> String {
    let messages = evaluation
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
        .take(4)
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>();
    if messages.is_empty() {
        "Runtime publish gate rejected the current project".to_string()
    } else {
        format!(
            "Runtime publish gate rejected publish: {}",
            messages.join("; ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_page_builder::RuntimeContextScenario;
    use serde_json::json;

    fn sample_controller() -> AdminCanvasController {
        let project_data = json!({
            "pages": [{
                "id": "home",
                "component": {
                    "type": "wrapper",
                    "components": []
                }
            }]
        });
        AdminCanvasController::new("home", "rev-1", project_data).expect("controller")
    }

    #[test]
    fn async_session_rejects_late_success_and_failure_from_an_identical_retry() {
        let controller = sample_controller();
        let mut session = EditorSessionState::new(controller, "Missing", "Saved");
        let hash = session.controller.editor().revision().project_hash;
        let first = session.mark_save_started_with_id().expect("first start");
        assert!(session.handle_publish_error_for_attempt(&first, "network failure"));
        let retry = session.mark_save_started_with_id().expect("retry");
        assert_ne!(first, retry);
        assert!(!session.handle_publish_error_for_attempt(&first, "stale error"));
        assert!(session.handle_publish_response_for_attempt(
            "home", "old-revision", Some(hash), &first,
        ).is_err());
        assert_eq!(session.controller.active_save_attempt().as_ref(), Some(&retry));
        assert_eq!(session.last_error.as_deref(), Some("network failure"));
        session.handle_publish_response_for_attempt(
            "home", "rev-2", Some(hash), &retry,
        ).expect("accepted response");
        assert_eq!(session.controller.revision_id(), "rev-2");
        assert!(session.last_error.is_none());
    }

    #[test]
    fn server_revision_conflict_retires_only_the_matching_attempt_and_fences_edits() {
        let controller = sample_controller();
        let mut session = EditorSessionState::new(controller, "Missing", "Saved");
        let attempt = session.mark_save_started_with_id().expect("save started");

        assert!(session.handle_publish_conflict_for_attempt(
            &attempt,
            "The page changed in another session",
        ));
        assert!(session.controller.active_save_attempt().is_none());
        assert!(session.controller.ui().state.dirty.save_conflict);
        assert!(!session.controller.ui().state.capabilities.edit);
        assert!(!session.handle_publish_error_for_attempt(
            &attempt,
            "late transport error",
        ));
    }

    #[test]
    fn peer_write_conflict_retires_active_save_before_late_acknowledgement() {
        let controller = sample_controller();
        let mut session = EditorSessionState::new(controller, "Missing", "Saved");
        let hash = session.controller.editor().revision().project_hash;
        let attempt = session.mark_save_started_with_id().expect("save started");

        assert!(session.handle_peer_write_conflict("rev-2"));
        assert!(session.controller.active_save_attempt().is_none());
        assert!(session.controller.ui().state.dirty.save_conflict);
        assert!(!session.controller.ui().state.capabilities.edit);
        assert!(session.last_error.as_deref().is_some_and(|error| error.contains("Another tab")));

        assert!(session
            .handle_publish_response_for_attempt("home", "rev-2", Some(hash), &attempt)
            .is_err());
        assert!(session.controller.ui().state.dirty.save_conflict);
        assert!(session.last_error.as_deref().is_some_and(|error| error.contains("Another tab")));
    }

    #[test]
    fn conflict_revision_is_extracted_from_pages_and_receipt_errors() {
        assert_eq!(
            conflict_actual_revision(
                "Page document changed concurrently: expected revision `rev-1`, found `rev-2`"
            )
            .as_deref(),
            Some("rev-2")
        );
        assert_eq!(
            conflict_actual_revision(
                "Pages document advanced after this save receipt: committed revision `rev-2`, current revision `rev-3`"
            )
            .as_deref(),
            Some("rev-3")
        );
    }

    #[test]
    fn editor_session_initializes_with_clean_defaults() {
        let controller = sample_controller();
        let session = EditorSessionState::new(controller, "Missing facade", "Save succeeded");
        assert_eq!(session.controller.page_id(), "home");
        assert!(session.last_error.is_none());
        assert!(session.last_announcement.is_none());
        assert!(!session.preview_in_progress);
    }

    #[test]
    fn editor_session_applies_runtime_scenarios() {
        let controller = sample_controller();
        let scenario = RuntimeContextScenario::new(
            "vip_user",
            "VIP User",
            json!({ "user": { "role": "vip" } }),
        )
        .with_description("A VIP customer scenario");
        let mut session = EditorSessionState::new(controller, "Missing", "Saved")
            .with_runtime_scenarios(Arc::new(vec![scenario]));

        let applied = session.apply_runtime_scenario("vip_user");
        assert!(applied);
        assert_eq!(session.active_runtime_scenario.as_deref(), Some("vip_user"));
        assert_eq!(session.runtime_context["user"]["role"], "vip");
        assert!(session.last_announcement.is_some());
    }

    #[test]
    fn editor_session_action_reducer_handles_announcements_and_errors() {
        let controller = sample_controller();
        let mut session = EditorSessionState::new(controller, "Missing", "Saved");

        session.dispatch(EditorSessionAction::Announce(
            "Draft auto-saved".to_string(),
        ));
        assert_eq!(
            session.last_announcement.as_deref(),
            Some("Draft auto-saved")
        );

        session.dispatch(EditorSessionAction::Fail("Save rejected".to_string()));
        assert_eq!(session.last_error.as_deref(), Some("Save rejected"));

        session.dispatch(EditorSessionAction::ClearError);
        assert!(session.last_error.is_none());
    }

    #[test]
    fn host_configuration_updates_runtime_inputs_and_retires_stale_preview() {
        let mut state = EditorSessionState::new(sample_controller(), "facade missing", "saved");
        state.runtime_context = serde_json::json!({ "tenant": "old" });
        state.runtime_context_configured = true;
        state.active_runtime_scenario = Some("old-scenario".to_string());
        state.server_preview_html = Some("<main>old</main>".to_string());
        state.preview_request = Some(PreviewAttempt::new(
            state.controller.editor().revision().project_hash,
            0,
            serde_json::json!({ "tenant": "old" }),
            Some("old-scenario".to_string()),
        ));
        state.preview_in_progress = true;
        state.fail("facade missing");

        state.update_host_configuration(
            None,
            None,
            Some(serde_json::json!({ "tenant": "new" })),
            None,
            None,
            true,
        );

        assert_eq!(state.runtime_context, serde_json::json!({ "tenant": "new" }));
        assert!(state.runtime_context_configured);
        assert!(state.active_runtime_scenario.is_none());
        assert!(state.runtime_publish_gate_evaluation.is_none());
        assert!(state.server_preview_html.is_none());
        assert!(state.preview_request.is_none());
        assert!(!state.preview_in_progress);
        assert!(state.last_error.is_none());
    }

    #[test]
    fn runtime_context_change_retires_preview_and_ignores_old_completion() {
        let mut state = EditorSessionState::new(sample_controller(), "Missing", "Saved");
        state.prepare_server_preview_request().expect("first preview");
        let first = state.preview_request.as_ref().expect("first attempt").id;

        state.set_runtime_context(json!({ "locale": "fr-FR" }));
        assert!(!state.preview_in_progress);
        assert!(state.preview_request.is_none());

        state.prepare_server_preview_request().expect("second preview");
        let second = state.preview_request.as_ref().expect("second attempt").id;
        assert_ne!(first, second);

        let stale = PreviewPageBuilderResult {
            page_id: "home".to_string(),
            html: "<main>stale</main>".to_string(),
            runtime_scenario_id: None,
        };
        state.handle_preview_failure(first, "late failure".to_string());
        state.handle_preview_response(first, &stale).expect("stale response ignored");
        assert!(state.preview_attempt_active(second));
        assert!(state.last_error.is_none());
    }

    #[test]
    fn mutating_intent_retires_an_in_flight_preview() {
        let mut state = EditorSessionState::new(sample_controller(), "Missing", "Saved");
        state.prepare_server_preview_request().expect("preview");

        state.dispatch_intent(UiIntent::Undo).expect("undo intent");

        assert!(!state.preview_in_progress);
        assert!(state.preview_request.is_none());
    }

    #[test]
    fn page_activation_retires_an_in_flight_preview() {
        let mut state = EditorSessionState::new(sample_controller(), "Missing", "Saved");
        state.prepare_server_preview_request().expect("preview");
        let attempt = state.preview_request.as_ref().expect("attempt").id;

        state.dispatch_intent(UiIntent::ActivatePage {
            page_id: Some("home".to_string()),
            page_index: 0,
        }).expect("activate page");

        assert!(!state.preview_in_progress);
        assert!(state.preview_request.is_none());
        state.handle_preview_failure(attempt, "late failure".to_string());
        assert!(state.last_error.is_none());
    }

    #[test]
    fn stale_preview_completion_cannot_retire_an_identical_retry() {
        let mut state = EditorSessionState::new(sample_controller(), "Missing", "Saved");
        state.prepare_server_preview_request().expect("first preview");
        let first = state.preview_request.as_ref().expect("attempt").id;
        state.update_host_configuration(
            None,
            None,
            Some(json!({"changed": true})),
            None,
            None,
            true,
        );
        state.update_host_configuration(None, None, None, None, None, true);
        state.prepare_server_preview_request().expect("second preview");
        let second = state.preview_request.as_ref().expect("attempt").id;
        assert_ne!(first, second);
        let response = PreviewPageBuilderResult {
            page_id: "home".to_string(),
            html: "<main>stale</main>".to_string(),
            runtime_scenario_id: None,
        };
        state.handle_preview_failure(first, "stale error".to_string());
        state.handle_preview_response(first, &response).expect("ignore stale");
        assert!(state.preview_attempt_active(second));
        assert!(state.last_error.is_none());
        assert!(state.server_preview_html.is_none());

        let response = PreviewPageBuilderResult {
            html: "<main>current</main>".to_string(),
            ..response
        };
        state.handle_preview_response(second, &response).expect("current preview");
        assert!(!state.preview_in_progress);
        assert_eq!(state.server_preview_html.as_deref(), Some("<main>current</main>"));
    }

    #[test]
    fn removing_host_trait_registry_restores_builtin_schemas() {
        let mut state = EditorSessionState::new(sample_controller(), "Missing", "Saved")
            .with_trait_schemas(Arc::new(TraitSchemaRegistry::default()));
        state.update_host_configuration(None, None, None, None, None, true);
        assert_eq!(state.trait_schemas.as_ref(), &TraitSchemaRegistry::with_builtins());
    }

    #[test]
    fn editor_session_prepares_preview_request() {
        let controller = sample_controller();
        let mut session = EditorSessionState::new(controller, "Missing", "Saved");

        let effects = session.dispatch(EditorSessionAction::RequestServerPreview);
        assert_eq!(effects.len(), 1);
        assert!(session.preview_in_progress);
        assert!(matches!(
            &effects[0],
            EditorSessionEffect::ExecuteRequest {
                request: PageBuilderCapabilityRequest::Preview(_),
                ..
            }
        ));
    }
}
