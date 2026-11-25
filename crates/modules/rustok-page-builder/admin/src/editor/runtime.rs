#[cfg(not(target_arch = "wasm32"))]
use crate::capability_access::intent_allowed;
use crate::core::session::PreviewAttempt;
use crate::{
    AdminCanvasController, AdminCanvasEffect, PageBuilderAdminFacade,
    PageBuilderAdminFacadeError, SaveAttempt,
};
use fly::{ProjectHash, TraitSchemaRegistry};
#[cfg(not(target_arch = "wasm32"))]
use fly_browser::BrowserIntentKind;
use fly_ui::{EditorCapability, EditorCapabilityEvaluation, UiIntent};
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_page_builder::dto::{
    BuilderCapabilityKind, PageBuilderCapabilityRequest, PageBuilderCapabilityResponse,
    PageBuilderPreviewRuntime, PreviewPageBuilderInput,
};
use rustok_page_builder::{
    RuntimeContextScenario, RuntimePublishGateEvaluation, RuntimePublishGatePolicy,
};
use serde_json::{Map, Value};
use std::sync::Arc;
#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
use fly_web::{BrowserPeerWriteChannel, BrowserPeerWriteConflict};

#[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
fn make_peer_write_channel(
    controller: RwSignal<AdminCanvasController>,
    peer_write_conflict: RwSignal<Option<BrowserPeerWriteConflict>>,
    last_error: RwSignal<Option<String>>,
    scope_id: Option<String>,
) -> Option<BrowserPeerWriteChannel> {
    let (page_id, revision_id, project_hash) = controller.with_untracked(|controller| {
        (
            controller.page_id().to_string(),
            controller.revision_id().to_string(),
            controller.editor().revision().project_hash.hex(),
        )
    });
    let on_conflict = move |conflict| {
        let expected = controller.with_untracked(|controller| controller.revision_id().to_string());
        let actual = conflict.revision.clone();
        if let Some(Err(error)) = controller.try_update(|controller| {
            controller.mark_peer_write_conflict(expected, actual)
        }) {
            tracing::warn!(?error, "Failed to mark peer-tab save conflict");
        }
        peer_write_conflict.set(Some(conflict.clone()));
        last_error.set(Some(format!(
            "Another tab saved this page at revision {}. Preserve your local changes and reload before editing again.",
            conflict.revision
        )));
    };
    let channel = match scope_id {
        Some(scope_id) => BrowserPeerWriteChannel::new_scoped(
            page_id,
            revision_id,
            project_hash,
            scope_id,
            on_conflict,
        ),
        None => BrowserPeerWriteChannel::new(
            page_id,
            revision_id,
            project_hash,
            on_conflict,
        ),
    };
    match channel {
        Ok(channel) => Some(channel),
        Err(error) => {
            tracing::warn!(?error, "Cross-tab write notifications are unavailable; server revision checks remain authoritative");
            None
        }
    }
}

/// A host callback receives the exact local attempt identity alongside its request.
/// This keeps callback-backed adapters as safe as the built-in facade path.
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

#[derive(Clone)]
pub struct PageBuilderCapabilityExecution {
    pub request: PageBuilderCapabilityRequest,
    pub save_attempt: Option<SaveAttempt>,
    /// Complete this exact request after the host finishes its asynchronous work.
    /// Call once; stale save completions are ignored by the captured request identity.
    pub on_complete: Callback<Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError>>,
}

impl PageBuilderCapabilityExecution {
    /// Deliver the host result through the same validation and acknowledgement path as a facade.
    pub fn complete(
        &self,
        outcome: Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError>,
    ) {
        self.on_complete.run(outcome);
    }
}

#[derive(Clone)]
pub struct AdminEditorRuntime {
    pub controller: RwSignal<AdminCanvasController>,
    pub last_error: RwSignal<Option<String>>,
    pub last_announcement: RwSignal<Option<String>>,
    pub server_preview_html: RwSignal<Option<String>>,
    pub preview_in_progress: RwSignal<bool>,
    pub trait_schemas: Arc<TraitSchemaRegistry>,
    pub editor_capability_evaluation: Option<Arc<EditorCapabilityEvaluation>>,
    pub runtime_context: RwSignal<Value>,
    pub runtime_context_configured: RwSignal<bool>,
    pub runtime_scenarios: Arc<Vec<RuntimeContextScenario>>,
    pub active_runtime_scenario: RwSignal<Option<String>>,
    pub runtime_publish_gate_policy: Option<Arc<RuntimePublishGatePolicy>>,
    pub runtime_publish_gate_evaluation: RwSignal<Option<RuntimePublishGateEvaluation>>,
    preview_request: RwSignal<Option<PreviewAttempt>>,
    facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    on_request: Option<Callback<PageBuilderCapabilityExecution>>,
    facade_missing: String,
    save_succeeded: String,
    /// The Rust Leptos workspace dispatches saves outside the classic fly-browser JS adapter.
    /// Share its confirmed writes through the same advisory channel so sibling tabs are fenced.
    #[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
    peer_write_channel: RwSignal<Option<BrowserPeerWriteChannel>>,
    #[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
    peer_write_conflict: RwSignal<Option<BrowserPeerWriteConflict>>,
}

impl AdminEditorRuntime {
    pub fn new(
        controller: AdminCanvasController,
        facade: Option<Arc<dyn PageBuilderAdminFacade>>,
        on_request: Option<Callback<PageBuilderCapabilityExecution>>,
        peer_write_scope_id: Option<String>,
        facade_missing: impl Into<String>,
        save_succeeded: impl Into<String>,
    ) -> Self {
        #[cfg(not(all(target_arch = "wasm32", feature = "wasm-client")))]
        let _ = peer_write_scope_id;
        let controller = RwSignal::new(controller);
        let last_error = RwSignal::new(None);
        #[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
        let peer_write_conflict = RwSignal::new(None);
        #[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
        let peer_write_channel = RwSignal::new(make_peer_write_channel(
            controller.clone(),
            peer_write_conflict.clone(),
            last_error.clone(),
            peer_write_scope_id,
        ));

        Self {
            controller,
            last_error,
            last_announcement: RwSignal::new(None),
            server_preview_html: RwSignal::new(None),
            preview_in_progress: RwSignal::new(false),
            trait_schemas: Arc::new(TraitSchemaRegistry::with_builtins()),
            editor_capability_evaluation: None,
            runtime_context: RwSignal::new(Value::Object(Map::new())),
            runtime_context_configured: RwSignal::new(false),
            runtime_scenarios: Arc::new(Vec::new()),
            active_runtime_scenario: RwSignal::new(None),
            runtime_publish_gate_policy: None,
            runtime_publish_gate_evaluation: RwSignal::new(None),
            preview_request: RwSignal::new(None),
            facade,
            on_request,
            facade_missing: facade_missing.into(),
            save_succeeded: save_succeeded.into(),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
            peer_write_channel,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
            peer_write_conflict,
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

    /// Whether another tab has committed a newer version of this page.
    pub fn peer_write_conflicted(&self) -> bool {
        #[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
        {
            self.peer_write_conflict.get_untracked().is_some()
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "wasm-client")))]
        {
            false
        }
    }

    pub fn reject_peer_write_conflict(&self) -> bool {
        if !self.peer_write_conflicted() {
            return false;
        }
        self.fail(
            "Another tab saved this page. Preserve your unsaved changes, then reload the latest version before editing again.",
        );
        true
    }

    fn broadcast_confirmed_write(&self, revision_id: &str, project_hash: ProjectHash) {
        #[cfg(all(target_arch = "wasm32", feature = "wasm-client"))]
        {
            if self.peer_write_conflicted() {
                return;
            }
            let hash = project_hash.hex();
            if let Some(Err(error)) = self.peer_write_channel.with_untracked(|channel| {
                channel
                    .as_ref()
                    .map(|channel| channel.post_confirmed_write(revision_id, &hash))
            }) {
                tracing::warn!(?error, "Failed to broadcast confirmed Page Builder save");
            }
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "wasm-client")))]
        {
            let _ = (revision_id, project_hash);
        }
    }

    pub fn capability_enabled(&self, capability: EditorCapability) -> bool {
        self.controller
            .with(|controller| controller.ui().state.capabilities.allows(capability))
    }

    /// Whether the intent endpoint admits the form that posts `intent`, by the rule the endpoint
    /// applies. A server-rendered form renders disabled when it does not, and an intent name
    /// the endpoint does not know is not admitted.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn form_enabled(&self, intent: &str) -> bool {
        self.form_enabled_with(intent, &Value::Null)
    }

    /// As [`Self::form_enabled`], for a form whose requirement depends on a field it posts.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn form_enabled_with(&self, intent: &str, payload: &Value) -> bool {
        BrowserIntentKind::parse(intent).is_some_and(|kind| {
            self.controller.with(|controller| {
                intent_allowed(kind, payload, controller.ui().state.capabilities)
            })
        })
    }

    pub fn with_runtime_context(mut self, runtime_context: Value) -> Self {
        self.runtime_context = RwSignal::new(runtime_context);
        self.runtime_context_configured = RwSignal::new(true);
        self.active_runtime_scenario = RwSignal::new(None);
        self.runtime_publish_gate_evaluation = RwSignal::new(None);
        self
    }

    pub fn with_runtime_scenarios(
        mut self,
        runtime_scenarios: Arc<Vec<RuntimeContextScenario>>,
    ) -> Self {
        self.runtime_scenarios = runtime_scenarios;
        self.runtime_publish_gate_evaluation = RwSignal::new(None);
        self
    }

    pub fn with_runtime_publish_gate_policy(
        mut self,
        policy: Arc<RuntimePublishGatePolicy>,
    ) -> Self {
        self.runtime_publish_gate_policy = Some(policy);
        self.runtime_publish_gate_evaluation = RwSignal::new(None);
        self
    }

    pub fn apply_runtime_scenario(&self, scenario_id: &str) -> bool {
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
        self.runtime_context.set(scenario.context.clone());
        self.runtime_context_configured.set(true);
        self.active_runtime_scenario.set(Some(scenario.id.clone()));
        self.runtime_publish_gate_evaluation.set(None);
        self.server_preview_html.set(None);
        self.preview_request.set(None);
        self.preview_in_progress.set(false);
        self.last_error.set(None);
        self.announce(format!("Preview scenario applied: {}", scenario.label));
        true
    }

    pub fn set_runtime_context(&self, runtime_context: Value) {
        self.runtime_context.set(runtime_context);
        self.runtime_context_configured.set(true);
        self.active_runtime_scenario.set(None);
        self.runtime_publish_gate_evaluation.set(None);
        self.server_preview_html.set(None);
        self.preview_request.set(None);
        self.preview_in_progress.set(false);
    }

    pub fn evaluate_runtime_publish_gate(&self) -> Option<RuntimePublishGateEvaluation> {
        let policy = self.runtime_publish_gate_policy.as_ref()?;
        let context = self.runtime_context.get_untracked();
        let configured = self.runtime_context_configured.get_untracked();
        Some(self.controller.with(|controller| {
            rustok_page_builder::evaluate_host_runtime_publish_gate(
                controller.editor().document(),
                configured.then_some(&context),
                self.runtime_scenarios.as_slice(),
                policy.as_ref(),
            )
        }))
    }

    pub fn request_server_preview(&self) {
        if self.preview_in_progress.get_untracked() {
            return;
        }
        let runtime_context = if self.runtime_context_configured.get_untracked() {
            self.runtime_context.get_untracked()
        } else {
            Value::Object(Map::new())
        };
        let runtime_scenario_id = self.active_runtime_scenario.get_untracked();
        let request = self.controller.with(|controller| {
            let active_page_index = controller.active_page_index();
            controller.active_page_project()
                .map(|project_data| {
                    (
                        PageBuilderCapabilityRequest::Preview(
                            PreviewPageBuilderInput::new(controller.page_id(), project_data)
                                .with_runtime(PageBuilderPreviewRuntime::new(
                                    runtime_context.clone(),
                                    runtime_scenario_id.clone(),
                                )),
                        ),
                        controller.editor().revision().project_hash,
                        active_page_index,
                        runtime_context.clone(),
                        runtime_scenario_id.clone(),
                    )
                })
                .map_err(|error| error.to_string())
        });
        match request {
            Ok((request, project_hash, active_page_index, runtime_context, scenario_id)) => {
                self.preview_request.set(Some(PreviewAttempt::new(
                    project_hash,
                    active_page_index,
                    runtime_context,
                    scenario_id,
                )));
                let sequence = self.controller.with_untracked(|controller| {
                    controller.editor().revision().command_sequence
                });
                self.execute_request(request, None, sequence);
            }
            Err(error) => self.fail(error),
        }
    }

    pub fn dispatch(&self, intent: UiIntent) {
        if intent.mutates_document() || matches!(&intent, UiIntent::ActivatePage { .. }) {
            self.runtime_publish_gate_evaluation.set(None);
            self.server_preview_html.set(None);
            self.preview_request.set(None);
            self.preview_in_progress.set(false);
        }
        if self.peer_write_conflicted() && intent.mutates_document() {
            self.reject_peer_write_conflict();
            return;
        }

        let result = self
            .controller
            .try_update(|controller| controller.dispatch(intent));
        let Some(result) = result else {
            self.last_error.set(Some(
                "Page Builder controller is no longer available".to_string(),
            ));
            return;
        };
        match result {
            Ok(effects) => {
                self.last_error.set(None);
                for effect in effects {
                    self.apply_effect(effect);
                }
            }
            Err(error) => self.last_error.set(Some(error.to_string())),
        }
    }

    /// Route the effects produced by a trusted Leptos workspace intent to
    /// the same consumer facade as toolbar commands. Merely dispatching the
    /// intent changed local state but silently dropped Publish/Preview before.
    pub fn apply_browser_result(&self, result: crate::BrowserIntentDispatchResult) {
        let in_scope = self.controller.with(|controller| {
            result.page_id == controller.page_id()
                && result.revision_id == controller.revision_id()
                && result.project_hash == controller.editor().revision().project_hash.hex()
                && result.command_sequence == controller.editor().revision().command_sequence
        });
        if !in_scope {
            self.fail("Browser workspace response no longer matches the active document");
            return;
        }
        self.last_error.set(None);
        for effect in result.effects {
            match effect {
                crate::BrowserIntentEffect::Announce { message } => {
                    self.announce(message);
                }
                crate::BrowserIntentEffect::Request {
                    request,
                    expected_hash,
                    command_sequence,
                } => {
                    let (actual_hash, actual_sequence, active_page_index) =
                        self.controller.with(|controller| {
                            (
                                controller.editor().revision().project_hash,
                                controller.editor().revision().command_sequence,
                                controller.active_page_index(),
                            )
                        });
                    if command_sequence != actual_sequence {
                        self.fail("Browser editor request is stale; refresh the current draft");
                        continue;
                    }
                    let expected_hash = match expected_hash {
                        Some(hex) if hex == actual_hash.hex() => Some(actual_hash),
                        Some(_) => {
                            self.fail("Browser editor request has a mismatched project hash");
                            continue;
                        }
                        None => None,
                    };
                    if request.capability() == BuilderCapabilityKind::Publish
                        && self.reject_peer_write_conflict()
                    {
                        continue;
                    }
                    if let PageBuilderCapabilityRequest::Preview(preview) = &request {
                        if self.preview_in_progress.get_untracked() {
                            self.fail("Preview already in progress");
                            continue;
                        }
                        if let Err(error) = self.controller.with(|controller| {
                            controller.validate_preview_request(preview, command_sequence)
                        }) {
                            self.fail(error.to_string());
                            continue;
                        }
                        // The browser supplies its own runtime context. Retire any
                        // cached render/gate for different inputs before adopting it.
                        let context = preview.runtime.context.clone();
                        let scenario = preview.runtime.scenario_id.clone();
                        if !self.runtime_context_configured.get_untracked()
                            || self.runtime_context.get_untracked() != context
                            || self.active_runtime_scenario.get_untracked() != scenario
                        {
                            self.runtime_publish_gate_evaluation.set(None);
                            self.server_preview_html.set(None);
                        }
                        self.runtime_context.set(context.clone());
                        self.runtime_context_configured.set(true);
                        self.active_runtime_scenario.set(scenario.clone());
                        self.preview_request.set(Some(PreviewAttempt::new(
                            actual_hash,
                            active_page_index,
                            context,
                            scenario,
                        )));
                    }
                    self.execute_request(request, expected_hash, command_sequence);
                }
            }
        }
    }

    pub fn dispatch_result(&self, intent: Result<UiIntent, String>) {
        match intent {
            Ok(intent) => self.dispatch(intent),
            Err(error) => self.last_error.set(Some(error)),
        }
    }

    pub fn announce(&self, message: impl Into<String>) {
        self.last_announcement.set(Some(message.into()));
    }

    pub fn fail(&self, message: impl Into<String>) {
        self.last_error.set(Some(message.into()));
    }

    fn apply_effect(&self, effect: AdminCanvasEffect) {
        match effect {
            AdminCanvasEffect::Announce(message) => self.announce(message),
            AdminCanvasEffect::Request {
                request,
                expected_hash,
                command_sequence,
            } => self.execute_request(request, expected_hash, command_sequence),
        }
    }

    fn execute_request(
        &self,
        request: PageBuilderCapabilityRequest,
        expected_hash: Option<ProjectHash>,
        command_sequence: u64,
    ) {
        let capability = request.capability();
        if capability == BuilderCapabilityKind::Publish && self.reject_peer_write_conflict() {
            return;
        }
        if capability == BuilderCapabilityKind::Preview
            && self.facade
                .as_ref()
                .and_then(|facade| facade.provider_status())
                .is_some_and(|status| !status.preview_enabled())
        {
            self.preview_in_progress.set(false);
            self.preview_request.set(None);
            self.fail("Server preview is disabled by the consumer provider or health policy");
            return;
        }
        let save_attempt = if capability == BuilderCapabilityKind::Publish {
            if expected_hash.is_none() {
                self.fail("Publish request has no submitted project hash");
                return;
            }
            if self.facade.is_some() || self.on_request.is_some() {
                let start = self
                    .controller
                    .try_update(|controller| {
                        controller.begin_save_request(&request, expected_hash, command_sequence)
                    });
                match start {
                    Some(Ok(attempt_id)) => Some(attempt_id),
                    Some(Err(error)) => {
                        self.fail(error.to_string());
                        return;
                    }
                    None => {
                        self.fail("Page Builder controller is no longer available");
                        return;
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
        let preview_attempt_id = if capability == BuilderCapabilityKind::Preview {
            let Some(attempt) = self.preview_request.get_untracked() else {
                self.fail("Preview request has no tracked attempt");
                return;
            };
            self.preview_in_progress.set(true);
            Some(attempt.id)
        } else {
            None
        };

        if let Some(facade) = self.facade.as_ref() {
            let facade = Arc::clone(facade);
            let runtime = self.clone();
            spawn_local(async move {
                let outcome = facade.execute(request).await;
                runtime.complete_capability_request(
                    capability,
                    expected_hash,
                    save_attempt,
                    preview_attempt_id,
                    outcome,
                );
            });
        } else if let Some(callback) = self.on_request.as_ref() {
            // Callback-backed hosts used to receive a fire-and-forget notification: Preview was
            // cleared immediately and Publish stayed in save_in_progress forever. Give the host
            // the same completion path as a PageBuilderAdminFacade, with the exact attempt bound.
            let runtime = self.clone();
            let completion_hash = expected_hash.clone();
            let completion_attempt = save_attempt.clone();
            let on_complete = Callback::new(move |outcome| {
                runtime.complete_capability_request(
                    capability,
                    completion_hash.clone(),
                    completion_attempt.clone(),
                    preview_attempt_id,
                    outcome,
                );
            });
            callback.run(PageBuilderCapabilityExecution {
                request,
                save_attempt,
                on_complete,
            });
        } else {
            // A missing host must retire only the preview it failed to dispatch. A stale
            // dispatch must never clear a newer preview or replace its error message.
            if capability == BuilderCapabilityKind::Preview {
                let active = preview_attempt_id.is_some_and(|id| {
                    self.preview_in_progress.get_untracked()
                        && self.preview_request.get_untracked()
                            .is_some_and(|attempt| attempt.id == id)
                });
                if active {
                    self.preview_in_progress.set(false);
                    self.preview_request.set(None);
                    self.fail(self.facade_missing.clone());
                }
            } else {
                // SaveStarted is emitted before the host callback is selected. If neither a
                // facade nor a callback exists, retire that exact attempt instead of leaving the
                // editor permanently stuck in save_in_progress.
                self.finish_failed_request(capability, save_attempt.as_ref());
                self.fail(self.facade_missing.clone());
            }
        }
    }

    fn complete_capability_request(
        &self,
        capability: BuilderCapabilityKind,
        expected_hash: Option<ProjectHash>,
        save_attempt: Option<SaveAttempt>,
        preview_attempt_id: Option<uuid::Uuid>,
        outcome: Result<PageBuilderCapabilityResponse, PageBuilderAdminFacadeError>,
    ) {
        if capability == BuilderCapabilityKind::Preview
            && (!self.preview_in_progress.get_untracked()
                || preview_attempt_id.is_none()
                || self.preview_request.get_untracked().map(|attempt| attempt.id) != preview_attempt_id)
        {
            // An old callback (including a duplicate completion) must not
            // consume the correlation state or error of a newer preview.
            return;
        }
        match outcome {
            Ok(PageBuilderCapabilityResponse::Preview(response))
                if capability == BuilderCapabilityKind::Preview =>
            {
                let preview_request = self.preview_request.get_untracked();
                self.preview_request.set(None);
                self.preview_in_progress.set(false);
                let current_runtime_context =
                    if self.runtime_context_configured.get_untracked() {
                        self.runtime_context.get_untracked()
                    } else {
                        Value::Object(Map::new())
                    };
                let current_runtime_scenario = self.active_runtime_scenario.get_untracked();
                let current = self.controller.with(|controller| {
                    (
                        controller.page_id().to_string(),
                        controller.editor().revision().project_hash,
                        controller.active_page_index(),
                    )
                });
                if response.page_id != current.0 {
                    self.fail(format!(
                        "Page Builder preview returned page `{}` for `{}`",
                        response.page_id, current.0
                    ));
                    return;
                }
                if response.runtime_scenario_id != current_runtime_scenario {
                    self.fail(
                        "Page Builder preview returned a different runtime scenario; refresh the preview",
                    );
                    return;
                }
                if preview_request.is_none_or(|expected| {
                    expected.snapshot
                        != (
                            current.1,
                            current.2,
                            current_runtime_context,
                            current_runtime_scenario,
                        )
                }) {
                    self.fail(
                        "Page Builder project or runtime context changed while the server preview was rendering; refresh the preview",
                    );
                    return;
                }
                self.server_preview_html.set(Some(response.html));
                self.last_error.set(None);
                self.announce("Server preview refreshed");
            }
            Ok(PageBuilderCapabilityResponse::Publish(response))
                if capability == BuilderCapabilityKind::Publish =>
            {
                let result = self.controller.try_update(|controller| {
                    if response.page_id != controller.page_id() {
                        return Err(format!(
                            "Page Builder facade returned page `{}` for `{}`",
                            response.page_id,
                            controller.page_id()
                        ));
                    }
                    let Some(expected_hash) = expected_hash.as_ref() else {
                        return Err("Publish request has no submitted project hash".to_string());
                    };
                    let Some(attempt) = save_attempt.as_ref() else {
                        return Err("Publish request has no tracked attempt".to_string());
                    };
                    controller
                        .acknowledge_save_for_attempt(
                            expected_hash.clone(),
                            attempt,
                            response.revision_id.clone(),
                        )
                        .map_err(|error| error.to_string())
                });
                match result {
                    Some(Ok(())) => {
                        if !self.peer_write_conflicted() {
                            self.last_error.set(None);
                            self.announce(self.save_succeeded.clone());
                        }
                        if let Some(expected_hash) = expected_hash {
                            self.broadcast_confirmed_write(&response.revision_id, expected_hash);
                        }
                    }
                    Some(Err(error)) => {
                        if self.finish_failed_request(capability, save_attempt.as_ref()) {
                            self.fail(error);
                        }
                    }
                    None => self.fail(
                        "Page Builder controller was disposed before save acknowledgement",
                    ),
                }
            }
            Ok(response) => {
                if self.finish_failed_request(capability, save_attempt.as_ref()) {
                    self.fail(format!(
                        "Page Builder facade returned `{}` for a `{capability}` request",
                        response.capability()
                    ));
                }
            }
            Err(error) => {
                let completed = if capability == BuilderCapabilityKind::Publish
                    && error.stable_code.as_deref() == Some("REVISION_CONFLICT")
                {
                    self.finish_conflicted_request(
                        capability,
                        save_attempt.as_ref(),
                        &error.message,
                    )
                } else {
                    self.finish_failed_request(capability, save_attempt.as_ref())
                };
                if completed {
                    self.fail(error.to_string());
                }
            }
        }
    }

    fn finish_conflicted_request(
        &self,
        capability: BuilderCapabilityKind,
        attempt: Option<&SaveAttempt>,
        message: &str,
    ) -> bool {
        if capability != BuilderCapabilityKind::Publish {
            return false;
        }
        let Some(attempt) = attempt else {
            return false;
        };
        let expected = self.controller.with_untracked(|controller| {
            controller.revision_id().to_string()
        });
        let actual = conflict_actual_revision(message)
            .unwrap_or_else(|| "newer server revision".to_string());
        self.controller
            .try_update(|controller| {
                controller.mark_save_conflict_for_attempt(attempt, expected, actual)
            })
            .is_some_and(|result| result.unwrap_or(false))
    }

    /// False means the completion belongs to an obsolete attempt. In that
    /// case it must neither fail the newer save nor replace its UI message.
    fn finish_failed_request(
        &self,
        capability: BuilderCapabilityKind,
        attempt: Option<&SaveAttempt>,
    ) -> bool {
        match capability {
            BuilderCapabilityKind::Publish => {
                let Some(attempt) = attempt else {
                    return false;
                };
                self.controller
                    .try_update(|controller| controller.mark_save_failed_for_attempt(attempt))
                    .is_some_and(|result| result.unwrap_or(false))
            }
            BuilderCapabilityKind::Preview => {
                self.preview_in_progress.set(false);
                self.preview_request.set(None);
                true
            }
            BuilderCapabilityKind::Tree | BuilderCapabilityKind::Properties => true,
        }
    }

    pub fn to_session_state(&self) -> crate::core::EditorSessionState {
        crate::core::EditorSessionState {
            controller: self.controller.get_untracked(),
            last_error: self.last_error.get_untracked(),
            last_announcement: self.last_announcement.get_untracked(),
            server_preview_html: self.server_preview_html.get_untracked(),
            preview_in_progress: self.preview_in_progress.get_untracked(),
            trait_schemas: Arc::clone(&self.trait_schemas),
            editor_capability_evaluation: self.editor_capability_evaluation.clone(),
            runtime_context: self.runtime_context.get_untracked(),
            runtime_context_configured: self.runtime_context_configured.get_untracked(),
            runtime_scenarios: Arc::clone(&self.runtime_scenarios),
            active_runtime_scenario: self.active_runtime_scenario.get_untracked(),
            runtime_publish_gate_policy: self.runtime_publish_gate_policy.clone(),
            runtime_publish_gate_evaluation: self.runtime_publish_gate_evaluation.get_untracked(),
            preview_request: self.preview_request.get_untracked(),
            facade_missing: self.facade_missing.clone(),
            save_succeeded: self.save_succeeded.clone(),
        }
    }

    pub fn apply_session_state(&self, state: &crate::core::EditorSessionState) {
        self.controller.set(state.controller.clone());
        self.last_error.set(state.last_error.clone());
        self.last_announcement.set(state.last_announcement.clone());
        self.server_preview_html
            .set(state.server_preview_html.clone());
        self.preview_in_progress.set(state.preview_in_progress);
        self.runtime_context.set(state.runtime_context.clone());
        self.runtime_context_configured
            .set(state.runtime_context_configured);
        self.active_runtime_scenario
            .set(state.active_runtime_scenario.clone());
        self.runtime_publish_gate_evaluation
            .set(state.runtime_publish_gate_evaluation.clone());
        self.preview_request.set(state.preview_request.clone());
    }
}
