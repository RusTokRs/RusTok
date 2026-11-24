use crate::model::{AdminCanvasController, AdminCanvasEffect, SaveAttempt};
use fly::{GrapesJsCodec, ProjectHash, TraitSchemaRegistry, ValidationSeverity};
use fly_ui::{EditorCapability, EditorCapabilityEvaluation, UiIntent};
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
    pub preview_request: Option<(ProjectHash, usize, Value, Option<String>)>,
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
        let runtime_scenarios = runtime_scenarios.unwrap_or_else(|| Arc::new(Vec::new()));
        let runtime_inputs_changed = self.runtime_context != runtime_context
            || self.runtime_context_configured != runtime_context_configured
            || optional_arc_changed(&self.trait_schemas, &trait_schemas)
            || optional_arc_changed(
                &self.editor_capability_evaluation,
                &editor_capability_evaluation,
            )
            || optional_arc_changed(&self.runtime_scenarios, &runtime_scenarios)
            || optional_arc_changed(
                &self.runtime_publish_gate_policy,
                &runtime_publish_gate_policy,
            );

        self.trait_schemas =
            trait_schemas.unwrap_or_else(|| Arc::new(TraitSchemaRegistry::with_builtins()));
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
        document.project.pages = document
            .project
            .pages
            .get(active_page_index)
            .cloned()
            .into_iter()
            .collect();
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
        self.preview_request = Some((
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
        response: &PreviewPageBuilderResult,
    ) -> Result<(), String> {
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
            expected
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
        let expected = self.controller.revision_id().to_string();
        match self.controller.mark_save_conflict_for_attempt(
            attempt,
            expected,
            "newer server revision".to_string(),
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

    pub fn dispatch_intent(&mut self, intent: UiIntent) -> Result<Vec<AdminCanvasEffect>, String> {
        if matches!(
            &intent,
            UiIntent::Execute(_) | UiIntent::Undo | UiIntent::Redo | UiIntent::ActivatePage { .. }
        ) {
            self.runtime_publish_gate_evaluation = None;
            self.server_preview_html = None;
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
                    self.fail(err);
                }
            }
            EditorSessionAction::SaveFailed { attempt, message } => {
                self.mark_save_failed_for_attempt(&attempt, message);
            }
            EditorSessionAction::PreviewAcknowledged(response) => {
                if let Err(err) = self.handle_preview_response(&response) {
                    self.fail(err);
                }
            }
            EditorSessionAction::PreviewFailed(err) => {
                self.preview_in_progress = false;
                self.preview_request = None;
                self.fail(err);
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
    PreviewAcknowledged(Box<PreviewPageBuilderResult>),
    PreviewFailed(String),
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
        assert!(session.controller.ui_state().dirty.save_conflict);
        assert!(!session.controller.ui_state().capabilities.edit);
        assert!(!session.handle_publish_error_for_attempt(
            &attempt,
            "late transport error",
        ));
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
        state.preview_request = Some((
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
