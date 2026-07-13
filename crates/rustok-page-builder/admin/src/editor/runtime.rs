use crate::{AdminCanvasController, AdminCanvasEffect, PageBuilderAdminFacade};
use fly::{ProjectHash, TraitSchemaRegistry};
use fly_ui::UiIntent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_page_builder::dto::{PageBuilderCapabilityRequest, PageBuilderCapabilityResponse};
use serde_json::{Map, Value};
use std::sync::Arc;

#[derive(Clone)]
pub struct AdminEditorRuntime {
    pub controller: RwSignal<AdminCanvasController>,
    pub last_error: RwSignal<Option<String>>,
    pub last_announcement: RwSignal<Option<String>>,
    pub trait_schemas: Arc<TraitSchemaRegistry>,
    pub runtime_context: RwSignal<Value>,
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
            trait_schemas: Arc::new(TraitSchemaRegistry::with_builtins()),
            runtime_context: RwSignal::new(Value::Object(Map::new())),
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

    pub fn with_runtime_context(mut self, runtime_context: Value) -> Self {
        self.runtime_context = RwSignal::new(runtime_context);
        self
    }

    pub fn dispatch(&self, intent: UiIntent) {
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
        if let Some(facade) = self.facade.as_ref() {
            let facade = Arc::clone(facade);
            let runtime = self.clone();
            let expected_hash = expected_hash.or_else(|| {
                runtime
                    .controller
                    .with(|controller| controller.ui().state.dirty.project_hash)
            });
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

            spawn_local(async move {
                match facade.execute(request).await {
                    Ok(PageBuilderCapabilityResponse::Publish(response)) => {
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
                        runtime.mark_save_failed();
                        runtime.fail(format!(
                            "Page Builder facade returned `{}` for a publish request",
                            response.capability()
                        ));
                    }
                    Err(error) => {
                        runtime.mark_save_failed();
                        runtime.fail(error.to_string());
                    }
                }
            });
        } else if let Some(callback) = self.on_request.as_ref() {
            callback.run(request);
        } else {
            self.fail(self.facade_missing.clone());
        }
    }

    fn mark_save_failed(&self) {
        let _ = self
            .controller
            .try_update(|controller| controller.mark_save_failed());
    }
}
