use fly::{
    EditorCommand, FlyEditor, GrapesJsCodec, PageLocator, PageSummary, ProjectFragment,
    ProjectHash, RegistrySet, ValidationReport,
};
use fly_ui::{FlyUiStateMachine, Presentation, UiEffect, UiIntent};
use rustok_page_builder::dto::{PageBuilderCapabilityRequest, PublishPageBuilderInput};
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
        let document = GrapesJsCodec::decode_value(project_data)?;
        let editor = FlyEditor::new(document, RegistrySet::with_builtins());
        let mut ui = FlyUiStateMachine::new(Presentation::Full);
        let summaries = editor.document().page_summaries();
        let active_page_index = summaries
            .iter()
            .position(|page| page.id.as_deref() == Some(page_id.as_str()))
            .unwrap_or(0);
        if let Some(active) = summaries.get(active_page_index) {
            ui.state.page.active_page_index = active_page_index;
            ui.state.page.active_page_id = active.id.clone();
        }
        let mut controller = Self {
            page_id,
            revision_id: revision_id.into(),
            editor,
            ui,
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

    pub fn page_summaries(&self) -> Vec<PageSummary> {
        self.editor.document().page_summaries()
    }

    pub fn active_page_index(&self) -> usize {
        self.ui.state.page.active_page_index
    }

    pub fn active_page_locator(&self) -> PageLocator {
        PageLocator::by_index(self.active_page_index())
    }

    pub fn active_page_summary(&self) -> Option<PageSummary> {
        self.page_summaries().get(self.active_page_index()).cloned()
    }

    pub fn active_root_id(&self) -> Option<String> {
        self.editor
            .document()
            .project
            .pages
            .get(self.active_page_index())
            .and_then(|page| page.component.as_ref())
            .and_then(|root| root.id())
            .map(ToString::to_string)
    }

    pub fn dispatch(
        &mut self,
        intent: UiIntent,
    ) -> Result<Vec<AdminCanvasEffect>, AdminCanvasError> {
        self.validate_navigation_intent(&intent)?;
        let snapshot = self.clone();
        let effects = self.ui.dispatch(intent)?;
        let mut outgoing = Vec::new();

        for effect in effects {
            if let Err(error) = self.apply_ui_effect(effect, &mut outgoing) {
                *self = snapshot;
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

    fn validate_navigation_intent(&self, intent: &UiIntent) -> Result<(), AdminCanvasError> {
        let UiIntent::ActivatePage {
            page_id,
            page_index,
        } = intent
        else {
            return Ok(());
        };
        let page = self
            .editor
            .document()
            .project
            .pages
            .get(*page_index)
            .ok_or_else(|| {
                AdminCanvasError::Authoring(format!("page index {page_index} does not exist"))
            })?;
        if page_id.is_some() && page.id.as_ref() != page_id.as_ref() {
            return Err(AdminCanvasError::Authoring(format!(
                "page navigation id {:?} does not match page at index {page_index}",
                page_id
            )));
        }
        Ok(())
    }

    fn apply_ui_effect(
        &mut self,
        effect: UiEffect,
        outgoing: &mut Vec<AdminCanvasEffect>,
    ) -> Result<(), AdminCanvasError> {
        match effect {
            UiEffect::None => Ok(()),
            UiEffect::Announce(message) => {
                outgoing.push(AdminCanvasEffect::Announce(message));
                Ok(())
            }
            UiEffect::Command(command) => {
                let report = self.editor.apply(*command)?;
                self.synchronize(report);
                Ok(())
            }
            UiEffect::Undo => {
                self.editor.undo()?;
                let report = self.editor.validate();
                self.synchronize(report);
                Ok(())
            }
            UiEffect::Redo => {
                self.editor.redo()?;
                let report = self.editor.validate();
                self.synchronize(report);
                Ok(())
            }
            UiEffect::CopySelection => {
                self.copy_selection()?;
                outgoing.push(AdminCanvasEffect::Announce("Component copied".to_string()));
                Ok(())
            }
            UiEffect::CutSelection => {
                self.cut_selection()?;
                outgoing.push(AdminCanvasEffect::Announce("Component cut".to_string()));
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
            } => {
                let project_data = GrapesJsCodec::encode_value(self.editor.document())?;
                outgoing.push(AdminCanvasEffect::Request {
                    request: PageBuilderCapabilityRequest::Publish(PublishPageBuilderInput {
                        page_id: self.page_id.clone(),
                        revision_id: self.revision_id.clone(),
                        project_data,
                    }),
                    expected_hash,
                    command_sequence,
                });
                Ok(())
            }
        }
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
            .ok_or_else(|| {
                AdminCanvasError::Authoring("selected component has no location".to_string())
            })?;
        if location.page_index != self.active_page_index() {
            return Err(AdminCanvasError::Authoring(
                "selected component is outside the active page".to_string(),
            ));
        }
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
        let child_count = self
            .editor
            .document()
            .child_count_for_parent(parent_id.as_deref())
            .ok_or_else(|| {
                AdminCanvasError::Authoring("clipboard parent is missing or opaque".to_string())
            })?;
        if index > child_count {
            return Err(AdminCanvasError::Authoring(format!(
                "clipboard insertion index {index} exceeds child count {child_count}"
            )));
        }
        for component_type in &component_types {
            if !self.editor.registries().accepts_child_type(
                parent_id
                    .as_deref()
                    .and_then(|parent_id| self.editor.document().component_type_for_id(parent_id)),
                component_type,
            ) {
                return Err(AdminCanvasError::Authoring(format!(
                    "clipboard component `{component_type}` is not accepted by the target"
                )));
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

    fn insertion_target(
        &self,
        child_type: &str,
    ) -> Result<(Option<String>, usize), AdminCanvasError> {
        let document = self.editor.document();
        let registries = self.editor.registries();
        match self.ui.state.selection.component_id.as_deref() {
            Some(selected_id) => {
                let location = document.component_location(selected_id).ok_or_else(|| {
                    AdminCanvasError::Authoring(format!(
                        "selected component `{selected_id}` has no location"
                    ))
                })?;
                if location.page_index != self.active_page_index() {
                    return Err(AdminCanvasError::Authoring(
                        "selected component is outside the active page".to_string(),
                    ));
                }
                let selected = document.component(selected_id).ok_or_else(|| {
                    AdminCanvasError::Authoring(format!(
                        "selected component `{selected_id}` does not exist"
                    ))
                })?;
                if location.depth == 0
                    || registries.accepts_child_type(Some(selected.component_type()), child_type)
                {
                    Ok((Some(selected_id.to_string()), selected.children().len()))
                } else {
                    Ok((
                        location.parent_component_id,
                        location.index.saturating_add(1),
                    ))
                }
            }
            None => {
                let root_id = self.active_root_id().ok_or_else(|| {
                    AdminCanvasError::Authoring(
                        "active page does not contain an editable root component".to_string(),
                    )
                })?;
                let child_count = document.component_child_count(&root_id).ok_or_else(|| {
                    AdminCanvasError::Authoring("active page root is opaque or missing".to_string())
                })?;
                Ok((Some(root_id), child_count))
            }
        }
    }

    fn synchronize(&mut self, report: ValidationReport) {
        self.synchronize_page_navigation();
        let active_page_index = self.active_page_index();
        let selection = self.editor.selection().and_then(|component_id| {
            self.editor
                .document()
                .component_location(component_id)
                .filter(|location| location.page_index == active_page_index)
                .map(|_| component_id.to_string())
        });
        if selection.is_none() && self.editor.selection().is_some() {
            let _ = self
                .editor
                .apply(EditorCommand::Select { component_id: None });
        }
        self.ui.state.selection.component_id = selection;
        self.ui.state.set_diagnostics(report.diagnostics);
        self.synchronize_revision();
    }

    fn synchronize_page_navigation(&mut self) {
        let summaries = self.editor.document().page_summaries();
        if summaries.is_empty() {
            self.ui.state.page.active_page_id = None;
            self.ui.state.page.active_page_index = 0;
            return;
        }
        let current_id = self.ui.state.page.active_page_id.as_deref();
        let index = current_id
            .and_then(|id| {
                summaries
                    .iter()
                    .position(|summary| summary.id.as_deref() == Some(id))
            })
            .unwrap_or_else(|| {
                self.ui
                    .state
                    .page
                    .active_page_index
                    .min(summaries.len().saturating_sub(1))
            });
        self.ui.state.page.active_page_index = index;
        self.ui.state.page.active_page_id = summaries[index].id.clone();
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
    use fly::{ComponentPatch, EditorCommand, PageCommand, blank_page};
    use serde_json::{Map, json};

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
            .dispatch(UiIntent::execute(EditorCommand::Patch {
                component_id: "hero".to_string(),
                patch: ComponentPatch {
                    attributes: Map::from_iter([("aria-label".to_string(), json!("Hero"))]),
                    ..ComponentPatch::default()
                },
            }))
            .expect("patch");
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
        assert_eq!(
            input.project_data["pages"][0]["component"]["components"][0]["attributes"]["aria-label"],
            "Hero"
        );
    }

    #[test]
    fn failed_command_restores_ui_editor_and_clipboard() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::Select(Some("copy-me".to_string())))
            .expect("select");
        controller.dispatch(UiIntent::CopySelection).expect("copy");
        let snapshot = controller.clone();
        assert!(
            controller
                .dispatch(UiIntent::execute(EditorCommand::Patch {
                    component_id: "missing".to_string(),
                    patch: ComponentPatch::default(),
                }))
                .is_err()
        );
        assert_eq!(controller.ui(), snapshot.ui());
        assert_eq!(controller.editor(), snapshot.editor());
        assert_eq!(controller.has_clipboard(), snapshot.has_clipboard());
    }

    #[test]
    fn copy_cut_and_paste_use_internal_fragment_with_new_ids() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::Select(Some("copy-me".to_string())))
            .expect("select");
        controller.dispatch(UiIntent::CopySelection).expect("copy");
        controller
            .dispatch(UiIntent::PasteClipboard)
            .expect("paste");
        let selected = controller
            .editor()
            .selection()
            .expect("pasted selection")
            .to_string();
        assert_ne!(selected, "copy-me");
        assert!(controller.editor().document().contains_component(&selected));
        assert!(controller.editor().document().contains_component("copy-me"));
        controller
            .dispatch(UiIntent::CutSelection)
            .expect("cut pasted component");
        assert!(!controller.editor().document().contains_component(&selected));
    }

    #[test]
    fn page_navigation_is_non_mutating_and_scopes_selection() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::execute(EditorCommand::Page {
                command: PageCommand::Add {
                    index: 1,
                    page: Box::new(blank_page("about", "About")),
                },
            }))
            .expect("add page");
        controller.acknowledge_save("rev-2").expect("acknowledge");
        controller
            .dispatch(UiIntent::ActivatePage {
                page_id: Some("about".to_string()),
                page_index: 1,
            })
            .expect("activate page");
        assert_eq!(controller.active_page_index(), 1);
        assert_eq!(controller.editor().selection(), None);
        assert!(!controller.ui().state.dirty.dirty);
    }
}
