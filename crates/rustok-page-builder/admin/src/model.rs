use fly::{
    FlyEditor, GrapesJsV1Codec, ProjectHash, RegistrySet, ValidationReport,
};
use fly_ui::{FlyUiStateMachine, Presentation, UiEffect, UiIntent};
use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PageBuilderContractMetadata, PublishPageBuilderInput,
};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct AdminCanvasController {
    page_id: String,
    revision_id: String,
    editor: FlyEditor,
    ui: FlyUiStateMachine,
}

impl AdminCanvasController {
    pub fn new(
        page_id: impl Into<String>,
        revision_id: impl Into<String>,
        project_data: Value,
    ) -> Result<Self, AdminCanvasError> {
        let page_id = page_id.into();
        if page_id.trim().is_empty() {
            return Err(AdminCanvasError::InvalidPageId);
        }
        let document = GrapesJsV1Codec::decode_value(project_data)?;
        let editor = FlyEditor::new(document, RegistrySet::with_builtins());
        let mut controller = Self {
            page_id,
            revision_id: revision_id.into(),
            editor,
            ui: FlyUiStateMachine::new(Presentation::Full),
        };
        let report = controller.editor.validate();
        controller.synchronize(report);
        Ok(controller)
    }

    pub fn page_id(&self) -> &str {
        &self.page_id
    }

    pub fn revision_id(&self) -> &str {
        &self.revision_id
    }

    pub fn editor(&self) -> &FlyEditor {
        &self.editor
    }

    pub fn ui(&self) -> &FlyUiStateMachine {
        &self.ui
    }

    pub fn can_undo(&self) -> bool {
        self.editor.history().can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.editor.history().can_redo()
    }

    pub fn dispatch(
        &mut self,
        intent: UiIntent,
    ) -> Result<Vec<AdminCanvasEffect>, AdminCanvasError> {
        let effects = self.ui.dispatch(intent)?;
        let mut outgoing = Vec::new();

        for effect in effects {
            match effect {
                UiEffect::None => {}
                UiEffect::Announce(message) => {
                    outgoing.push(AdminCanvasEffect::Announce(message));
                }
                UiEffect::Command(command) => {
                    let report = self.editor.apply(command)?;
                    self.synchronize(report);
                }
                UiEffect::Undo => {
                    self.editor.undo()?;
                    let report = self.editor.validate();
                    self.synchronize(report);
                }
                UiEffect::Redo => {
                    self.editor.redo()?;
                    let report = self.editor.validate();
                    self.synchronize(report);
                }
                UiEffect::Persist {
                    expected_hash,
                    command_sequence,
                } => {
                    let project_data = GrapesJsV1Codec::encode_value(self.editor.document())?;
                    outgoing.push(AdminCanvasEffect::Request {
                        request: PageBuilderCapabilityRequest::Publish(PublishPageBuilderInput {
                            page_id: self.page_id.clone(),
                            revision_id: self.revision_id.clone(),
                            schema_version: PageBuilderContractMetadata::BASELINE
                                .contract
                                .to_string(),
                            project_data,
                        }),
                        expected_hash,
                        command_sequence,
                    });
                }
            }
        }

        Ok(outgoing)
    }

    pub fn mark_save_started(&mut self) -> Result<(), AdminCanvasError> {
        self.editor.revision_mut().begin_save();
        self.ui.dispatch(UiIntent::SaveStarted)?;
        self.synchronize_revision();
        Ok(())
    }

    pub fn mark_save_failed(&mut self) -> Result<(), AdminCanvasError> {
        self.editor.revision_mut().fail_save();
        self.ui.dispatch(UiIntent::SaveFailed)?;
        self.synchronize_revision();
        Ok(())
    }

    pub fn acknowledge_save(
        &mut self,
        revision_id: impl Into<String>,
    ) -> Result<(), AdminCanvasError> {
        let revision_id = revision_id.into();
        let project_hash = self.editor.revision().project_hash;
        self.editor
            .revision_mut()
            .acknowledge(project_hash, revision_id.clone())?;
        self.revision_id = revision_id.clone();
        self.ui.dispatch(UiIntent::SaveSucceeded {
            revision: revision_id,
            project_hash,
        })?;
        self.synchronize_revision();
        Ok(())
    }

    fn synchronize(&mut self, report: ValidationReport) {
        self.ui.state.selection.component_id =
            self.editor.selection().map(ToString::to_string);
        self.ui.state.set_diagnostics(report.diagnostics);
        self.synchronize_revision();
    }

    fn synchronize_revision(&mut self) {
        let revision = self.editor.revision();
        self.ui.state.dirty.dirty = revision.dirty;
        self.ui.state.dirty.command_sequence = revision.command_sequence;
        self.ui.state.dirty.last_acknowledged_revision =
            revision.last_acknowledged_revision.clone();
        self.ui.state.dirty.project_hash = Some(revision.project_hash);
        self.ui.state.dirty.save_in_progress = revision.save_in_progress;
        self.ui.state.dirty.save_failed = revision.save_failed;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AdminCanvasEffect {
    Request {
        request: PageBuilderCapabilityRequest,
        expected_hash: Option<ProjectHash>,
        command_sequence: u64,
    },
    Announce(String),
}

#[derive(Debug, thiserror::Error)]
pub enum AdminCanvasError {
    #[error("page id must not be empty")]
    InvalidPageId,
    #[error(transparent)]
    Fly(#[from] fly::FlyError),
    #[error(transparent)]
    Ui(#[from] fly_ui::UiError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::{ComponentPatch, EditorCommand};
    use serde_json::{json, Map};

    fn controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{
                            "id": "hero",
                            "type": "section",
                            "components": []
                        }]
                    }
                }]
            }),
        )
        .expect("controller")
    }

    #[test]
    fn mutation_and_save_emit_canonical_publish_request() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::Execute(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch {
                    attributes: Map::from_iter([(
                        "aria-label".to_string(),
                        json!("Hero"),
                    )]),
                    ..ComponentPatch::default()
                },
            }))
            .expect("patch");
        assert!(controller.ui().state.dirty.dirty);

        let effects = controller
            .dispatch(UiIntent::RequestSave)
            .expect("request save");
        let AdminCanvasEffect::Request { request, .. } = &effects[0] else {
            panic!("expected request effect");
        };
        let PageBuilderCapabilityRequest::Publish(input) = request else {
            panic!("expected publish request");
        };
        assert_eq!(input.page_id, "home");
        assert_eq!(input.schema_version, "grapesjs_v1");
        assert_eq!(input.project_data["pages"][0]["component"]["components"][0]["attributes"]["aria-label"], "Hero");
    }

    #[test]
    fn save_acknowledgement_clears_dirty_state() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::Execute(EditorCommand::Select {
                component_id: Some("hero".to_string()),
            }))
            .expect("select");
        controller.acknowledge_save("rev-2").expect("acknowledge");
        assert_eq!(controller.revision_id(), "rev-2");
        assert!(!controller.ui().state.dirty.dirty);
    }
}
