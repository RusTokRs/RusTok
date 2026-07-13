use fly::{
    EditorCommand, FlyEditor, GrapesJsV1Codec, ProjectFragment, ProjectHash, RegistrySet,
    ValidationReport,
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
    clipboard: Option<ProjectFragment>,
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
            clipboard: None,
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

    pub fn has_clipboard(&self) -> bool {
        self.clipboard.is_some()
    }

    pub fn dispatch(
        &mut self,
        intent: UiIntent,
    ) -> Result<Vec<AdminCanvasEffect>, AdminCanvasError> {
        let previous_ui = self.ui.clone();
        let effects = self.ui.dispatch(intent)?;
        let mut outgoing = Vec::new();

        for effect in effects {
            let result: Result<(), AdminCanvasError> = match effect {
                UiEffect::None => Ok(()),
                UiEffect::Announce(message) => {
                    outgoing.push(AdminCanvasEffect::Announce(message));
                    Ok(())
                }
                UiEffect::Command(command) => match self.editor.apply(command) {
                    Ok(report) => {
                        self.synchronize(report);
                        Ok(())
                    }
                    Err(error) => Err(error.into()),
                },
                UiEffect::Undo => match self.editor.undo() {
                    Ok(_) => {
                        let report = self.editor.validate();
                        self.synchronize(report);
                        Ok(())
                    }
                    Err(error) => Err(error.into()),
                },
                UiEffect::Redo => match self.editor.redo() {
                    Ok(_) => {
                        let report = self.editor.validate();
                        self.synchronize(report);
                        Ok(())
                    }
                    Err(error) => Err(error.into()),
                },
                UiEffect::CopySelection => {
                    self.copy_selection()?;
                    outgoing.push(AdminCanvasEffect::Announce(
                        "Component copied".to_string(),
                    ));
                    Ok(())
                }
                UiEffect::CutSelection => {
                    self.cut_selection()?;
                    outgoing.push(AdminCanvasEffect::Announce(
                        "Component cut".to_string(),
                    ));
                    Ok(())
                }
                UiEffect::PasteClipboard => {
                    let inserted = self.paste_clipboard()?;
                    outgoing.push(AdminCanvasEffect::Announce(format!(
                        "Pasted {} component(s)",
                        inserted.len()
                    )));
                    Ok(())
                }
                UiEffect::Persist {
                    expected_hash,
                    command_sequence,
                } => match GrapesJsV1Codec::encode_value(self.editor.document()) {
                    Ok(project_data) => {
                        outgoing.push(AdminCanvasEffect::Request {
                            request: PageBuilderCapabilityRequest::Publish(
                                PublishPageBuilderInput {
                                    page_id: self.page_id.clone(),
                                    revision_id: self.revision_id.clone(),
                                    schema_version: PageBuilderContractMetadata::BASELINE
                                        .contract
                                        .to_string(),
                                    project_data,
                                },
                            ),
                            expected_hash,
                            command_sequence,
                        });
                        Ok(())
                    }
                    Err(error) => Err(error.into()),
                },
            };

            if let Err(error) = result {
                self.ui = previous_ui;
                return Err(error);
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
        let project_hash = self.editor.revision().project_hash;
        self.acknowledge_save_for_hash(project_hash, revision_id)
    }

    pub fn acknowledge_save_for_hash(
        &mut self,
        expected_hash: ProjectHash,
        revision_id: impl Into<String>,
    ) -> Result<(), AdminCanvasError> {
        let revision_id = revision_id.into();
        self.editor
            .revision_mut()
            .acknowledge(expected_hash, revision_id.clone())?;
        self.revision_id = revision_id.clone();
        self.ui.dispatch(UiIntent::SaveSucceeded {
            revision: revision_id,
            project_hash: expected_hash,
        })?;
        self.synchronize_revision();
        Ok(())
    }

    fn copy_selection(&mut self) -> Result<(), AdminCanvasError> {
        let component_id = self
            .editor
            .selection()
            .ok_or_else(|| AdminCanvasError::Authoring("no component is selected".to_string()))?;
        let location = self
            .editor
            .document()
            .component_location(component_id)
            .ok_or_else(|| AdminCanvasError::Authoring("selected component has no location".to_string()))?;
        if location.depth == 0 {
            return Err(AdminCanvasError::Authoring(
                "the page root cannot be copied into the internal clipboard".to_string(),
            ));
        }
        self.clipboard = Some(ProjectFragment::from_component(
            self.editor.document(),
            component_id,
        )?);
        Ok(())
    }

    fn cut_selection(&mut self) -> Result<(), AdminCanvasError> {
        self.copy_selection()?;
        let component_id = self
            .editor
            .selection()
            .ok_or_else(|| AdminCanvasError::Authoring("no component is selected".to_string()))?
            .to_string();
        let report = self.editor.apply(EditorCommand::Remove { component_id })?;
        self.synchronize(report);
        Ok(())
    }

    fn paste_clipboard(&mut self) -> Result<Vec<String>, AdminCanvasError> {
        let fragment = self.clipboard.clone().ok_or_else(|| {
            AdminCanvasError::Authoring("the internal clipboard is empty".to_string())
        })?;
        let component_types = fragment
            .components
            .iter()
            .map(|component| {
                component
                    .as_object()
                    .map(|component| component.component_type().to_string())
                    .ok_or_else(|| {
                        AdminCanvasError::Authoring(
                            "opaque clipboard components cannot be pasted interactively"
                                .to_string(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let first_type = component_types.first().ok_or_else(|| {
            AdminCanvasError::Authoring("the internal clipboard has no components".to_string())
        })?;
        let (parent_id, index) = self.insertion_target(first_type)?;
        for (offset, component_type) in component_types.iter().enumerate() {
            let decision = self.editor.registries().evaluate_placement(
                self.editor.document(),
                None,
                component_type,
                parent_id.as_deref(),
                index.saturating_add(offset),
            );
            if !decision.legal {
                return Err(AdminCanvasError::Authoring(
                    decision
                        .reason
                        .unwrap_or_else(|| "clipboard placement was rejected".to_string()),
                ));
            }
        }

        let inserted = fragment.insert(&mut self.editor, parent_id, index)?;
        if let Some(component_id) = inserted.first() {
            self.editor.apply(EditorCommand::Select {
                component_id: Some(component_id.clone()),
            })?;
        }
        let report = self.editor.validate();
        self.synchronize(report);
        Ok(inserted)
    }

    fn insertion_target(&self, child_type: &str) -> Result<(Option<String>, usize), AdminCanvasError> {
        let document = self.editor.document();
        let registries = self.editor.registries();
        let target = match self.ui.state.selection.component_id.as_deref() {
            Some(selected_id) => {
                let selected = document.component(selected_id).ok_or_else(|| {
                    AdminCanvasError::Authoring(format!(
                        "selected component `{selected_id}` does not exist"
                    ))
                })?;
                if registries.accepts_child_type(Some(selected.component_type()), child_type) {
                    (Some(selected_id.to_string()), selected.children().len())
                } else {
                    let location = document.component_location(selected_id).ok_or_else(|| {
                        AdminCanvasError::Authoring(format!(
                            "selected component `{selected_id}` has no location"
                        ))
                    })?;
                    if location.depth == 0 {
                        (None, document.root_child_count().unwrap_or_default())
                    } else {
                        (
                            location.parent_component_id,
                            location.index.saturating_add(1),
                        )
                    }
                }
            }
            None => (None, document.root_child_count().unwrap_or_default()),
        };
        Ok(target)
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
    #[error("{0}")]
    Authoring(String),
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
                            "components": [{
                                "id": "copy-me",
                                "type": "text",
                                "content": "Hello"
                            }]
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
    fn rejected_engine_command_rolls_back_ui_dirty_state() {
        let mut controller = controller();
        let previous = controller.ui().clone();
        let error = controller
            .dispatch(UiIntent::Execute(EditorCommand::Patch {
                component_id: "missing".to_string(),
                patch: ComponentPatch::default(),
            }))
            .expect_err("missing component must fail");
        assert!(matches!(error, AdminCanvasError::Fly(_)));
        assert_eq!(controller.ui(), &previous);
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

    #[test]
    fn stale_save_acknowledgement_keeps_newer_changes_dirty() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::Execute(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch {
                    attributes: Map::from_iter([("data-version".to_string(), json!("one"))]),
                    ..ComponentPatch::default()
                },
            }))
            .expect("first patch");
        let expected_hash = controller.editor().revision().project_hash;
        controller
            .dispatch(UiIntent::Execute(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch {
                    attributes: Map::from_iter([("data-version".to_string(), json!("two"))]),
                    ..ComponentPatch::default()
                },
            }))
            .expect("second patch");

        assert!(controller
            .acknowledge_save_for_hash(expected_hash, "rev-stale")
            .is_err());
        assert!(controller.ui().state.dirty.dirty);
        assert_eq!(controller.revision_id(), "rev-1");
    }

    #[test]
    fn copy_cut_and_paste_use_internal_fragment_with_new_ids() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::Select(Some("copy-me".to_string())))
            .expect("select");
        controller
            .dispatch(UiIntent::CopySelection)
            .expect("copy");
        assert!(controller.has_clipboard());
        controller
            .dispatch(UiIntent::PasteClipboard)
            .expect("paste");
        let selected = controller.editor().selection().expect("pasted selection");
        assert_ne!(selected, "copy-me");
        assert!(selected.starts_with("paste"));

        controller
            .dispatch(UiIntent::CutSelection)
            .expect("cut pasted component");
        assert!(!controller.editor().document().contains_component(selected));
    }
}
