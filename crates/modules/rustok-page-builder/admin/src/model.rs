use fly::{
    ContentDigest, EditorCommand, FlyEditor, GrapesJsCodec, PageLocator, PageSummary,
    ProjectFragment, ProjectHash, ProjectSnapshot, RegistrySet, ValidationReport,
};
use fly_ui::{FlyUiStateMachine, Presentation, UiEffect, UiIntent};
use rustok_page_builder::dto::{
    PageBuilderCapabilityRequest, PreviewPageBuilderInput, PublishPageBuilderInput,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Identity carried from a local save start through the asynchronous host response.
/// The sequence protects the in-memory Fly revision snapshot; the request ID remains unique
/// across controller replacement and rehydration, where the local sequence restarts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveAttempt {
    pub sequence: u64,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdminCanvasController {
    page_id: String,
    revision_id: String,
    editor: FlyEditor,
    ui: FlyUiStateMachine,
    clipboard: Option<ProjectFragment>,
    /// A request cannot be resumed after deserialization. Do not persist its identity.
    #[serde(skip)]
    active_save_request_id: Option<String>,
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
        let mut registries = RegistrySet::with_builtins();
        for definition in crate::asset_provider::builtin_asset_provider_definitions() {
            if let Err(error) = registries.asset_providers.register(definition) {
                tracing::warn!(?error, "Failed to register builtin asset provider definition");
            }
        }
        let editor = FlyEditor::new(document, registries);
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
            active_save_request_id: None,
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

    /// Encode the active page for preview without changing the editor or its revision.
    /// A missing active page or a serialization failure must not become an empty project.
    pub fn active_page_project(&self) -> fly::FlyResult<Value> {
        let index = self.active_page_index();
        let mut document = self.editor.document().clone();
        let page =
            document
                .project()
                .pages
                .get(index)
                .cloned()
                .ok_or(fly::FlyError::InvalidPageIndex {
                    index,
                    len: document.project().pages.len(),
                })?;
        document.project_mut().pages = vec![page];
        GrapesJsCodec::encode_value(&document)
    }

    /// Page navigation does not advance the document revision or hash. A delayed
    /// preview effect must therefore also match the actual active-page projection.
    pub(crate) fn validate_preview_request(
        &self,
        input: &PreviewPageBuilderInput,
        command_sequence: u64,
    ) -> Result<(), AdminCanvasError> {
        if input.page_id != self.page_id
            || command_sequence != self.editor.revision().command_sequence
            || input.project_data != self.active_page_project()?
        {
            return Err(AdminCanvasError::Authoring(
                "preview request no longer matches the active page snapshot".to_string(),
            ));
        }
        Ok(())
    }

    pub fn active_page_summary(&self) -> Option<PageSummary> {
        self.page_summaries().get(self.active_page_index()).cloned()
    }

    pub fn active_root_id(&self) -> Option<String> {
        self.editor
            .document()
            .project()
            .pages
            .get(self.active_page_index())
            .and_then(|page| page.component.as_ref())
            .and_then(|root| root.id())
            .map(ToString::to_string)
    }

    /// Apply a locally recovered document as one validated, undoable Fly command.
    /// The server revision is deliberately unchanged; the next save still uses its CAS token.
    pub fn apply_recovered_merge(
        &mut self,
        project_data: Value,
    ) -> Result<(), AdminCanvasError> {
        if self.editor.revision().save_conflict {
            return Err(AdminCanvasError::Authoring(
                "reload the conflicting draft before applying a recovered merge".to_string(),
            ));
        }
        let document = GrapesJsCodec::decode_value(project_data.clone())?;
        let snapshot = ProjectSnapshot {
            id: format!("local-recovery-{}", Uuid::new_v4()),
            label: "Recovered local draft".to_string(),
            project_hash: document.hash().hex(),
            content_digest: Some(ContentDigest::from_json(&project_data)?),
            project_data,
            metadata: Default::default(),
        };
        let sequence_before = self.editor.revision().command_sequence;
        let report = self.editor.restore_snapshot(&snapshot)?;
        self.clipboard = None;
        self.synchronize(report);
        self.invalidate_geometry_after_document_change(sequence_before);
        Ok(())
    }

    pub fn dispatch(
        &mut self,
        intent: UiIntent,
    ) -> Result<Vec<AdminCanvasEffect>, AdminCanvasError> {
        // Lifecycle transitions have to update the revision owner and UI
        // together. They are not general authoring intents.
        if matches!(
            &intent,
            UiIntent::SaveStarted { .. }
                | UiIntent::SaveSucceeded { .. }
                | UiIntent::SaveFailed { .. }
                | UiIntent::SaveConflict { .. }
        ) {
            return Err(AdminCanvasError::Authoring(
                "save lifecycle events require the correlated controller API".to_string(),
            ));
        }
        self.validate_navigation_intent(&intent)?;
        // A late hover from a removed component or a different page is not
        // a valid interaction target in the current canvas.
        let intent = match intent {
            UiIntent::Hover(Some(id)) if !self.component_on_active_page(&id) => UiIntent::Hover(None),
            intent => intent,
        };
        let snapshot = self.clone();
        let effects = match self.ui.dispatch(intent) {
            Ok(effects) => effects,
            Err(error) => {
                *self = snapshot;
                return Err(error.into());
            }
        };
        let mut outgoing = Vec::new();

        for effect in effects {
            if let Err(error) = self.apply_ui_effect(effect, &mut outgoing) {
                *self = snapshot;
                return Err(error);
            }
        }
        self.invalidate_geometry_after_document_change(snapshot.editor.revision().command_sequence);
        Ok(outgoing)
    }

    /// All canvas geometry refers to the previous document, even when IDs survive.
    fn invalidate_geometry_after_document_change(&mut self, sequence_before: u64) {
        if self.editor.revision().command_sequence != sequence_before {
            self.ui.state.invalidate_document_geometry();
        }
    }

    fn component_on_active_page(&self, component_id: &str) -> bool {
        self.editor.document().component_location(component_id)
            .is_some_and(|location| location.page_index == self.active_page_index())
    }

    /// Bind a transport attempt to the exact emitted persistence effect.
    /// Reject stale or mismatched payloads before invoking the consumer.
    pub fn begin_save_request(
        &mut self,
        request: &PageBuilderCapabilityRequest,
        expected_hash: Option<ProjectHash>,
        command_sequence: u64,
    ) -> Result<SaveAttempt, AdminCanvasError> {
        let PageBuilderCapabilityRequest::Publish(input) = request else {
            return Err(AdminCanvasError::Authoring("expected a save request".to_string()));
        };
        let revision = self.editor.revision();
        if input.page_id != self.page_id
            || input.revision_id != self.revision_id
            || expected_hash != Some(revision.project_hash)
            || command_sequence != revision.command_sequence
            || input.project_data != GrapesJsCodec::encode_value(self.editor.document())?
        {
            return Err(AdminCanvasError::Authoring(
                "save request no longer matches the active document snapshot".to_string(),
            ));
        }
        self.mark_save_started_with_id()
    }

    pub fn mark_save_started(&mut self) -> Result<(), AdminCanvasError> {
        self.mark_save_started_with_id().map(|_| ())
    }

    pub fn mark_save_started_with_id(&mut self) -> Result<SaveAttempt, AdminCanvasError> {
        let before = self.clone();
        let sequence = self.editor.revision_mut().begin_save_with_id()?;
        let request_id = Uuid::new_v4().to_string();
        if let Err(error) = self.ui.dispatch(UiIntent::SaveStarted {
            request_id: request_id.clone(),
        }) {
            *self = before;
            return Err(error.into());
        }
        self.active_save_request_id = Some(request_id.clone());
        self.synchronize_revision();
        Ok(SaveAttempt {
            sequence,
            request_id,
        })
    }

    /// The local sequence is useful for diagnostics, but is not sufficient to identify a
    /// request after this controller has been replaced or restored from serialized state.
    pub fn active_save_attempt_id(&self) -> Option<u64> {
        self.editor.revision().active_save_attempt_id()
    }

    pub fn active_save_attempt(&self) -> Option<SaveAttempt> {
        Some(SaveAttempt {
            sequence: self.editor.revision().active_save_attempt_id()?,
            request_id: self.active_save_request_id.clone()?,
        })
    }

    /// A late error from an old attempt must not fail a newer save, including after rehydration
    /// when the per-controller sequence can be reused.
    pub fn mark_save_failed_for_attempt(
        &mut self,
        attempt: &SaveAttempt,
    ) -> Result<bool, AdminCanvasError> {
        if self.active_save_attempt().as_ref() != Some(attempt) {
            return Ok(false);
        }
        let before = self.clone();
        self.editor.revision_mut().fail_save();
        if let Err(error) = self.ui.dispatch(UiIntent::SaveFailed {
            request_id: Some(attempt.request_id.clone()),
        }) {
            *self = before;
            return Err(error.into());
        }
        self.active_save_request_id = None;
        self.synchronize_revision();
        Ok(true)
    }

    /// Uncorrelated failures may update an idle controller, but can never retire an active save.
    pub fn mark_save_failed(&mut self) -> Result<(), AdminCanvasError> {
        if self.active_save_attempt().is_some() {
            return Err(AdminCanvasError::Authoring(
                "an active save failure must identify its request".to_string(),
            ));
        }
        let before = self.clone();
        self.editor.revision_mut().fail_save();
        if let Err(error) = self.ui.dispatch(UiIntent::SaveFailed { request_id: None }) {
            *self = before;
            return Err(error.into());
        }
        self.active_save_request_id = None;
        self.synchronize_revision();
        Ok(())
    }

    /// Record a lost update when no local save is in flight (for example, a peer-tab notice).
    /// Server responses must use mark_save_conflict_for_attempt.
    /// A peer-tab notification is not tied to a particular local request. Fence whichever
    /// attempt is active now, so a delayed response from it cannot acknowledge an older draft.
    pub fn mark_peer_write_conflict(
        &mut self,
        expected: String,
        actual: String,
    ) -> Result<(), AdminCanvasError> {
        let before = self.clone();
        self.editor.revision_mut().mark_save_conflict();
        if let Err(error) = self.ui.dispatch(UiIntent::SaveConflict {
            request_id: self.active_save_request_id.clone(),
            expected,
            actual,
        }) {
            *self = before;
            return Err(error.into());
        }
        self.active_save_request_id = None;
        self.synchronize_revision();
        Ok(())
    }

    pub fn mark_save_conflict(
        &mut self,
        expected: String,
        actual: String,
    ) -> Result<(), AdminCanvasError> {
        if self.active_save_attempt().is_some() {
            return Err(AdminCanvasError::Authoring(
                "an active save conflict must identify its request".to_string(),
            ));
        }
        let before = self.clone();
        self.editor.revision_mut().mark_save_conflict();
        if let Err(error) = self.ui.dispatch(UiIntent::SaveConflict {
            request_id: None,
            expected,
            actual,
        }) {
            *self = before;
            return Err(error.into());
        }
        self.active_save_request_id = None;
        self.synchronize_revision();
        Ok(())
    }

    pub fn mark_save_conflict_for_attempt(
        &mut self,
        attempt: &SaveAttempt,
        expected: String,
        actual: String,
    ) -> Result<bool, AdminCanvasError> {
        if self.active_save_attempt().as_ref() != Some(attempt) {
            return Ok(false);
        }
        let before = self.clone();
        self.editor.revision_mut().mark_save_conflict();
        if let Err(error) = self.ui.dispatch(UiIntent::SaveConflict {
            request_id: Some(attempt.request_id.clone()),
            expected,
            actual,
        }) {
            *self = before;
            return Err(error.into());
        }
        self.active_save_request_id = None;
        self.synchronize_revision();
        Ok(true)
    }

    /// Acknowledge only the exact attempt that produced the response. A matching hash or local
    /// sequence alone is not enough: both can repeat after controller rehydration.
    pub fn acknowledge_save_for_attempt(
        &mut self,
        expected_hash: ProjectHash,
        attempt: &SaveAttempt,
        revision_id: impl Into<String>,
    ) -> Result<(), AdminCanvasError> {
        let before = self.clone();
        if self.active_save_attempt().as_ref() != Some(attempt) {
            return Err(AdminCanvasError::Authoring(
                "save response does not match the active request".to_string(),
            ));
        }
        let revision_id = revision_id.into();
        self.editor.revision_mut().acknowledge_submitted_attempt(
            expected_hash,
            attempt.sequence,
            revision_id.clone(),
        )?;
        if let Err(error) = self.ui.dispatch(UiIntent::SaveSucceeded {
            request_id: attempt.request_id.clone(),
            revision: revision_id.clone(),
            project_hash: expected_hash,
        }) {
            *self = before;
            return Err(error.into());
        }
        self.revision_id = revision_id;
        self.active_save_request_id = None;
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
            .project()
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
            if let Err(err) = self
                .editor
                .apply(EditorCommand::Select { component_id: None })
            {
                tracing::warn!(error = ?err, "Failed to deselect out-of-scope component");
            }
        }
        self.ui.state.set_selected_component(selection);
        if self.ui.state.selection.hovered_component_id.as_deref()
            .is_some_and(|id| !self.component_on_active_page(id))
        {
            self.ui.state.set_hovered_component(None);
        }
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
        self.ui.state.dirty.save_started_request_id = self.active_save_request_id.clone();
        self.ui.state.dirty.save_failed = revision.save_failed;
        self.ui.state.dirty.save_conflict = revision.save_conflict;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

impl AdminCanvasController {
    /// Install host-composed provider registries after canonical project decode and immediately
    /// revalidate the document. The controller keeps the mutable Fly registry private; optional
    /// domain modules receive only this narrow installation seam.
    pub fn install_contribution_registries(
        &mut self,
        installer: impl FnOnce(&mut RegistrySet) -> Result<(), String>,
    ) -> Result<(), AdminCanvasError> {
        let mut candidate = self.editor.registries().clone();
        installer(&mut candidate).map_err(AdminCanvasError::Authoring)?;
        *self.editor.registries_mut() = candidate;
        let report = self.editor.validate();
        self.synchronize(report);
        Ok(())
    }
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
    fn geometry_is_invalidated_on_document_changes_but_not_rejected_commands() {
        let mut controller = controller();
        let rect = fly_ui::CanvasRect { x: 0.0, y: 0.0, width: 100.0, height: 40.0 };
        controller.dispatch(UiIntent::Select(Some("hero".into()))).unwrap();
        controller.dispatch(UiIntent::Hover(Some("copy-me".into()))).unwrap();
        controller.dispatch(UiIntent::SetSelectedOverlay(Some(rect))).unwrap();
        controller.dispatch(UiIntent::SetHoveredOverlay(Some(rect))).unwrap();
        let before = controller.clone();
        assert!(controller.dispatch(UiIntent::execute(EditorCommand::Remove {
            component_id: "missing".into(),
        })).is_err());
        assert_eq!(controller, before);

        controller.dispatch(UiIntent::execute(EditorCommand::Remove {
            component_id: "copy-me".into(),
        })).unwrap();
        assert!(controller.ui().state.selection.hovered_component_id.is_none());
        assert_eq!(controller.ui().state.overlays, fly_ui::OverlayState::default());
        controller.dispatch(UiIntent::Hover(Some("copy-me".into()))).unwrap();
        assert!(controller.ui().state.selection.hovered_component_id.is_none());

        controller.dispatch(UiIntent::Select(Some("hero".into()))).unwrap();
        controller.dispatch(UiIntent::SetSelectedOverlay(Some(rect))).unwrap();
        controller.dispatch(UiIntent::Undo).unwrap();
        assert_eq!(controller.ui().state.overlays, fly_ui::OverlayState::default());
    }

    #[test]
    fn failed_contribution_installation_preserves_controller_state() {
        let mut controller = controller();
        let before = controller.clone();
        let result = controller.install_contribution_registries(|registries| {
            *registries = RegistrySet::default();
            Err("installer failed after mutation".to_string())
        });
        assert!(matches!(result, Err(AdminCanvasError::Authoring(_))));
        assert_eq!(controller.editor(), before.editor());
        assert_eq!(controller.ui(), before.ui());
        assert_eq!(controller.revision_id(), before.revision_id());
    }

    #[test]
    fn save_request_admission_binds_payload_revision_and_sequence() {
        let mut controller = controller();
        let effects = controller.dispatch(UiIntent::RequestSave).expect("save effect");
        let AdminCanvasEffect::Request { request, expected_hash, command_sequence } = &effects[0] else {
            panic!("expected persistence effect");
        };
        for field in ["page", "revision", "payload", "hash", "sequence"] {
            let mut candidate = request.clone();
            let PageBuilderCapabilityRequest::Publish(input) = &mut candidate else {
                panic!("expected publish");
            };
            let mut hash = *expected_hash;
            let mut sequence = *command_sequence;
            match field {
                "page" => input.page_id = "other".to_string(),
                "revision" => input.revision_id = "old".to_string(),
                "payload" => input.project_data = json!({"pages": []}),
                "hash" => hash = None,
                "sequence" => sequence += 1,
                _ => unreachable!(),
            }
            let before = controller.clone();
            assert!(controller.begin_save_request(&candidate, hash, sequence).is_err(), "{field}");
            assert_eq!(controller, before);
        }
        let attempt = controller.begin_save_request(request, *expected_hash, *command_sequence)
            .expect("current effect");
        assert_eq!(controller.active_save_attempt().as_ref(), Some(&attempt));
        assert!(controller.mark_save_failed_for_attempt(&attempt).expect("retire"));
    }

    #[test]
    fn generic_dispatch_cannot_mutate_save_lifecycle() {
        let mut controller = controller();
        let before = controller.clone();
        assert!(controller.dispatch(UiIntent::SaveStarted {
            request_id: "unbound".into(),
        }).is_err());
        assert_eq!(controller, before);

        let attempt = controller.mark_save_started_with_id().expect("start");
        let hash = controller.editor().revision().project_hash;
        let before = controller.clone();
        for intent in [
            UiIntent::SaveSucceeded {
                request_id: attempt.request_id.clone(),
                revision: "rev-2".into(),
                project_hash: hash,
            },
            UiIntent::SaveFailed { request_id: Some(attempt.request_id.clone()) },
            UiIntent::SaveConflict {
                request_id: Some(attempt.request_id.clone()),
                expected: "rev-1".into(),
                actual: "rev-2".into(),
            },
        ] {
            assert!(controller.dispatch(intent).is_err());
            assert_eq!(controller, before);
        }
        assert!(controller.acknowledge_save_for_attempt(hash, &attempt, "").is_err());
        assert_eq!(controller, before);
        controller.acknowledge_save_for_attempt(hash, &attempt, "rev-2")
            .expect("correlated acknowledgement");
        assert_eq!(controller.revision_id(), "rev-2");
        assert!(!controller.ui().state.dirty.save_in_progress);
    }

    #[test]
    fn controller_rejects_stale_completions_of_identical_retries() {
        let mut controller = controller();
        let hash = controller.editor().revision().project_hash;
        let first = controller.mark_save_started_with_id().expect("first save");
        assert!(controller.mark_save_failed_for_attempt(&first).expect("failed"));
        let retry = controller.mark_save_started_with_id().expect("retry");
        assert_ne!(first, retry);
        assert!(!controller.mark_save_failed_for_attempt(&first).expect("stale failure"));
        assert!(controller.acknowledge_save_for_attempt(hash, &first, "stale").is_err());
        assert_eq!(controller.active_save_attempt().as_ref(), Some(&retry));
        assert_eq!(controller.revision_id(), "rev-1");
        controller.acknowledge_save_for_attempt(hash, &retry, "rev-2")
            .expect("current response");
        assert_eq!(controller.revision_id(), "rev-2");
    }

    #[test]
    fn recovered_merge_is_validated_dirty_and_undoable() {
        let mut controller = controller();
        let original_hash = controller.editor().revision().project_hash;
        let mut recovered =
            GrapesJsCodec::encode_value(controller.editor().document()).expect("encode");
        recovered["pages"][0]["component"]["components"][0]["components"][0]["content"] =
            serde_json::json!("Recovered offline edit");

        controller
            .apply_recovered_merge(recovered)
            .expect("apply recovered document");

        assert!(controller.editor().revision().dirty);
        assert_ne!(controller.editor().revision().project_hash, original_hash);
        let recovered_snapshot =
            GrapesJsCodec::encode_value(controller.editor().document()).expect("encode merged");
        assert_eq!(
            recovered_snapshot["pages"][0]["component"]["components"][0]["components"][0]
                ["content"],
            "Recovered offline edit"
        );
        assert_eq!(controller.revision_id(), "rev-1");

        controller.dispatch(UiIntent::Undo).expect("undo recovery");
        assert_eq!(controller.editor().revision().project_hash, original_hash);
    }

    #[test]
    fn peer_write_conflict_fences_an_idle_controller_too() {
        let mut controller = controller();
        controller
            .mark_peer_write_conflict("rev-1".into(), "rev-2".into())
            .expect("peer conflict");
        assert!(controller.active_save_attempt().is_none());
        assert!(controller.ui().state.dirty.save_conflict);
        assert!(controller.ui().state.dirty.dirty);
        assert_eq!(controller.revision_id(), "rev-1");
    }

    #[test]
    fn conflict_fence_survives_host_capability_refresh_without_changing_document() {
        let mut controller = controller();
        controller
            .mark_peer_write_conflict("rev-1".into(), "rev-2".into())
            .expect("peer conflict");
        controller.dispatch(UiIntent::SetEditableCapabilities(fly_ui::CapabilityState::full()))
            .expect("refresh host permissions");
        assert!(!controller.ui().state.capabilities.edit);
        let before = controller.clone();
        assert!(controller.dispatch(UiIntent::execute(EditorCommand::Remove {
            component_id: "hero".into(),
        })).is_err());
        let recovered = GrapesJsCodec::encode_value(controller.editor().document())
            .expect("recovery document");
        assert!(controller.apply_recovered_merge(recovered).is_err());
        assert_eq!(controller, before);
    }

    #[test]
    fn peer_write_conflict_retires_active_attempt_and_rejects_its_late_success() {
        let mut controller = controller();
        let hash = controller.editor().revision().project_hash;
        let attempt = controller.mark_save_started_with_id().expect("save started");

        controller
            .mark_peer_write_conflict("rev-1".into(), "rev-2".into())
            .expect("peer conflict");
        assert!(controller.active_save_attempt().is_none());
        assert_eq!(controller.revision_id(), "rev-1");
        assert!(controller.editor().revision().save_conflict);
        assert!(controller.ui().state.dirty.save_conflict);
        assert!(controller.acknowledge_save_for_attempt(hash, &attempt, "rev-2").is_err());
        assert_eq!(controller.revision_id(), "rev-1");
        assert!(controller.ui().state.dirty.dirty);
    }

    #[test]
    fn controller_acknowledges_only_submitted_snapshot_and_keeps_newer_edits() {
        let mut controller = controller();
        let patch = |label: &str| UiIntent::execute(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default()
                .set_attribute("aria-label", json!(label)),
        });
        controller.dispatch(patch("first")).expect("first edit");
        let submitted_hash = controller.editor().revision().project_hash;
        let first_attempt = controller.mark_save_started_with_id().expect("begin");
        assert!(controller.mark_save_started().is_err());
        controller.dispatch(patch("second")).expect("edit in flight");
        let working_hash = controller.editor().revision().project_hash;
        assert_ne!(submitted_hash, working_hash);
        controller.acknowledge_save_for_attempt(submitted_hash, &first_attempt, "rev-2")
            .expect("persisted earlier snapshot");
        assert!(controller.editor().revision().dirty);
        assert!(controller.ui().state.dirty.dirty);
        assert_eq!(controller.revision_id(), "rev-2");
        assert_eq!(controller.editor().revision().project_hash, working_hash);
        assert!(!controller.editor().revision().save_conflict);
        assert!(controller
            .acknowledge_save_for_attempt(submitted_hash, &first_attempt, "rev-3")
            .is_err());
        let latest_attempt = controller.mark_save_started_with_id().expect("save newest");
        controller.acknowledge_save_for_attempt(working_hash, &latest_attempt, "rev-3")
            .expect("persisted current snapshot");
        assert!(!controller.ui().state.dirty.dirty);
    }

    #[test]
    fn controller_request_identity_survives_attempt_sequence_reuse_after_rehydration() {
        let mut original = controller();
        let hash = original.editor().revision().project_hash;
        let old_attempt = original.mark_save_started_with_id().expect("old save");

        let serialized = serde_json::to_value(&original).expect("serialize active save");
        let mut restored: AdminCanvasController =
            serde_json::from_value(serialized).expect("restore controller");
        let new_attempt = restored.mark_save_started_with_id().expect("new save");
        assert_eq!(old_attempt.sequence, new_attempt.sequence);
        assert_ne!(old_attempt.request_id, new_attempt.request_id);
        assert!(restored
            .acknowledge_save_for_attempt(hash, &old_attempt, "stale-revision")
            .is_err());
        assert_eq!(restored.active_save_attempt().as_ref(), Some(&new_attempt));
        restored
            .acknowledge_save_for_attempt(hash, &new_attempt, "current-revision")
            .expect("current request remains acknowledgeable");
    }

    #[test]
    fn controller_failed_commands_do_not_leave_partial_save_state() {
        let mut controller = controller();
        let hash = controller.editor().revision().project_hash;
        let stale_attempt = SaveAttempt {
            sequence: 1,
            request_id: "stale-request".to_string(),
        };
        assert!(controller
            .acknowledge_save_for_attempt(hash, &stale_attempt, "rev-2")
            .is_err());
        assert_eq!(controller.revision_id(), "rev-1");
        controller.mark_save_conflict("rev-1".into(), "rev-2".into())
            .expect("conflict");
        assert!(controller.mark_save_started().is_err());
        assert!(!controller.ui().state.dirty.save_in_progress);
        assert!(!controller.editor().revision().save_in_progress);
        controller.mark_save_failed().expect("generic failure");
        assert!(controller.ui().state.dirty.save_conflict);
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
    fn history_transitions_discard_old_drop_candidates_but_failed_edits_do_not() {
        use fly_ui::{CanvasRect, DragSource, DropPosition, HitTestCandidate};

        let mut controller = controller();
        controller.dispatch(UiIntent::execute(EditorCommand::Patch {
            component_id: "hero".to_string(),
            patch: ComponentPatch::default().merge_style(json!({"margin": "12px"})),
        })).expect("edit");
        for history_intent in [UiIntent::Undo, UiIntent::Redo] {
            controller.dispatch(UiIntent::BeginDrag(DragSource::ExistingComponent {
                component_id: "copy-me".to_string(),
            })).expect("begin drag");
            controller.dispatch(UiIntent::UpdateHitTest(vec![HitTestCandidate {
                target_component_id: "hero".to_string(),
                parent_component_id: Some("root".to_string()),
                index: 0,
                position: DropPosition::Inside,
                rect: CanvasRect { x: 0.0, y: 0.0, width: 10.0, height: 10.0 },
                score: 1.0,
                legal: true,
                reason: None,
            }])).expect("hit test");
            let before = controller.clone();
            assert!(controller.dispatch(UiIntent::execute(EditorCommand::Remove {
                component_id: "missing".to_string(),
            })).is_err());
            assert_eq!(controller, before);

            controller.dispatch(history_intent).expect("history transition");
            assert!(controller.ui().state.drag.is_none());
            assert!(controller.ui().state.overlays.insertion.is_none());
            let after = controller.clone();
            assert!(controller.dispatch(UiIntent::Drop).is_err());
            assert_eq!(controller, after);
        }
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
    fn preview_request_admission_rejects_stale_page_even_without_revision_change() {
        let mut controller = controller();
        controller
            .dispatch(UiIntent::execute(EditorCommand::Page {
                command: PageCommand::Add {
                    index: 1,
                    page: Box::new(blank_page("about", "About")),
                },
            }))
            .expect("add page");
        let sequence = controller.editor().revision().command_sequence;
        let hash = controller.editor().revision().project_hash;
        let home = PreviewPageBuilderInput::new(
            controller.page_id(),
            controller.active_page_project().expect("home projection"),
        );
        controller.validate_preview_request(&home, sequence).expect("current home");
        controller.dispatch(UiIntent::ActivatePage {
            page_id: Some("about".into()),
            page_index: 1,
        }).expect("navigate");
        assert_eq!(controller.editor().revision().command_sequence, sequence);
        assert_eq!(controller.editor().revision().project_hash, hash);
        assert!(controller.validate_preview_request(&home, sequence).is_err());

        let about = PreviewPageBuilderInput::new(
            controller.page_id(),
            controller.active_page_project().expect("about projection"),
        );
        controller.validate_preview_request(&about, sequence).expect("current about");
        assert!(controller.validate_preview_request(&about, sequence + 1).is_err());
        let foreign = PreviewPageBuilderInput::new("foreign", about.project_data.clone());
        assert!(controller.validate_preview_request(&foreign, sequence).is_err());
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
        let submitted_hash = controller.editor().revision().project_hash;
        let attempt = controller
            .mark_save_started_with_id()
            .expect("save started");
        controller
            .acknowledge_save_for_attempt(submitted_hash, &attempt, "rev-2")
            .expect("acknowledge");
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
