use crate::{AdminCanvasController, AdminCanvasEffect, PageBuilderAdminFacade};
use fly::{
    GrapesJsCodec, ProjectHash, RuntimeContextScenario, RuntimePublishGateEvaluation,
    RuntimePublishGatePolicy, TraitSchemaRegistry, ValidationSeverity,
    evaluate_runtime_publish_gate,
};
use fly_ui::{EditorCapability, EditorCapabilityEvaluation, UiIntent};
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_page_builder::dto::{
    BuilderCapabilityKind, PageBuilderCapabilityRequest, PageBuilderCapabilityResponse,
    PreviewPageBuilderInput,
};
use serde_json::{Map, Value};
use std::sync::Arc;

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
    preview_request: RwSignal<Option<(ProjectHash, usize)>>,
    facade: Option<Arc<dyn PageBuilderAdminFacade>>,
    on_request: Option<Callback<PageBuilderCapabilityRequest>>,
    facade_missing: String,
    save_succeeded: String,
}

impl AdminEditorRuntime {
    pub fn new(
        controller: AdminCanvasController,
        facade: Option<Arc<dyn PageBuilderAdminFacade>>,
        on_request: Option<Callback<PageBuilderCapabilityRequest>>,
        facade_missing: impl Into<String>,
        save_succeeded: impl Into<String>,
    ) -> Self {
        Self {
            controller: RwSignal::new(controller),
            last_error: RwSignal::new(None),
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
        self.controller
            .with(|controller| controller.ui().state.capabilities.allows(capability))
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
    }

    pub fn evaluate_runtime_publish_gate(&self) -> Option<RuntimePublishGateEvaluation> {
        let policy = self.runtime_publish_gate_policy.as_ref()?;
        let context = self.runtime_context.get_untracked();
        let configured = self.runtime_context_configured.get_untracked();
        Some(self.controller.with(|controller| {
            evaluate_runtime_publish_gate(
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
        let request = self.controller.with(|controller| {
            let active_page_index = controller.active_page_index();
            let mut document = controller.editor().document().clone();
            document.project.pages = document
                .project
                .pages
                .get(active_page_index)
                .cloned()
                .into_iter()
                .collect();
            GrapesJsCodec::encode_value(&document)
                .map(|project_data| {
                    (
                        PageBuilderCapabilityRequest::Preview(PreviewPageBuilderInput::new(
                            controller.page_id(),
                            project_data,
                        )),
                        controller.editor().revision().project_hash,
                        active_page_index,
                    )
                })
                .map_err(|error| error.to_string())
        });
        match request {
            Ok((request, project_hash, active_page_index)) => {
                self.preview_request
                    .set(Some((project_hash, active_page_index)));
                self.execute_request(request, None);
            }
            Err(error) => self.fail(error),
        }
    }

    pub fn dispatch(&self, intent: UiIntent) {
        if matches!(
            &intent,
            UiIntent::Execute(_) | UiIntent::Undo | UiIntent::Redo | UiIntent::ActivatePage { .. }
        ) {
            self.runtime_publish_gate_evaluation.set(None);
            self.server_preview_html.set(None);
        }
        if matches!(&intent, UiIntent::RequestSave) {
            if let Some(evaluation) = self.evaluate_runtime_publish_gate() {
                let allowed = evaluation.allowed;
                let message = gate_error_message(&evaluation);
                self.runtime_publish_gate_evaluation.set(Some(evaluation));
                if !allowed {
                    self.fail(message);
                    return;
                }
            }
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
                ..
            } => self.execute_request(request, expected_hash),
        }
    }

    fn execute_request(
        &self,
        request: PageBuilderCapabilityRequest,
        expected_hash: Option<ProjectHash>,
    ) {
        let capability = request.capability();
        if let Some(facade) = self.facade.as_ref() {
            let facade = Arc::clone(facade);
            let runtime = self.clone();
            let expected_hash = expected_hash.or_else(|| {
                runtime
                    .controller
                    .with(|controller| controller.ui().state.dirty.project_hash)
            });
            if capability == BuilderCapabilityKind::Publish {
                let start = runtime
                    .controller
                    .try_update(|controller| controller.mark_save_started());
                match start {
                    Some(Ok(())) => {}
                    Some(Err(error)) => {
                        runtime.fail(error.to_string());
                        return;
                    }
                    None => {
                        runtime.fail("Page Builder controller is no longer available");
                        return;
                    }
                }
            } else if capability == BuilderCapabilityKind::Preview {
                runtime.preview_in_progress.set(true);
            }

            spawn_local(async move {
                match facade.execute(request).await {
                    Ok(PageBuilderCapabilityResponse::Preview(response))
                        if capability == BuilderCapabilityKind::Preview =>
                    {
                        let preview_request = runtime.preview_request.get_untracked();
                        runtime.preview_request.set(None);
                        runtime.preview_in_progress.set(false);
                        let current = runtime.controller.with(|controller| {
                            (
                                controller.page_id().to_string(),
                                controller.editor().revision().project_hash,
                                controller.active_page_index(),
                            )
                        });
                        if response.page_id != current.0 {
                            runtime.fail(format!(
                                "Page Builder preview returned page `{}` for `{}`",
                                response.page_id, current.0
                            ));
                            return;
                        }
                        if preview_request
                            .is_none_or(|expected| expected != (current.1, current.2))
                        {
                            runtime.fail(
                                "Page Builder project changed while the server preview was rendering; refresh the preview",
                            );
                            return;
                        }
                        runtime.server_preview_html.set(Some(response.html));
                        runtime.last_error.set(None);
                        runtime.announce("Server preview refreshed");
                    }
                    Ok(PageBuilderCapabilityResponse::Publish(response))
                        if capability == BuilderCapabilityKind::Publish =>
                    {
                        let result = runtime.controller.try_update(|controller| {
                            if response.page_id != controller.page_id() {
                                return Err(format!(
                                    "Page Builder facade returned page `{}` for `{}`",
                                    response.page_id,
                                    controller.page_id()
                                ));
                            }
                            let expected_hash = expected_hash
                                .unwrap_or(controller.editor().revision().project_hash);
                            controller
                                .acknowledge_save_for_hash(
                                    expected_hash,
                                    response.revision_id.clone(),
                                )
                                .map_err(|error| error.to_string())
                        });
                        match result {
                            Some(Ok(())) => {
                                runtime.last_error.set(None);
                                runtime.announce(runtime.save_succeeded.clone());
                            }
                            Some(Err(error)) => {
                                runtime.mark_save_failed();
                                runtime.fail(error);
                            }
                            None => runtime.fail(
                                "Page Builder controller was disposed before save acknowledgement",
                            ),
                        }
                    }
                    Ok(response) => {
                        runtime.finish_failed_request(capability);
                        runtime.fail(format!(
                            "Page Builder facade returned `{}` for a `{capability}` request",
                            response.capability()
                        ));
                    }
                    Err(error) => {
                        runtime.finish_failed_request(capability);
                        runtime.fail(error.to_string());
                    }
                }
            });
        } else if let Some(callback) = self.on_request.as_ref() {
            self.preview_in_progress.set(false);
            self.preview_request.set(None);
            callback.run(request);
        } else {
            self.preview_in_progress.set(false);
            self.preview_request.set(None);
            self.fail(self.facade_missing.clone());
        }
    }

    fn finish_failed_request(&self, capability: BuilderCapabilityKind) {
        match capability {
            BuilderCapabilityKind::Publish => self.mark_save_failed(),
            BuilderCapabilityKind::Preview => {
                self.preview_in_progress.set(false);
                self.preview_request.set(None);
            }
            BuilderCapabilityKind::Tree | BuilderCapabilityKind::Properties => {}
        }
    }

    fn mark_save_failed(&self) {
        let _ = self
            .controller
            .try_update(|controller| controller.mark_save_failed());
    }
}

fn gate_error_message(evaluation: &RuntimePublishGateEvaluation) -> String {
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
