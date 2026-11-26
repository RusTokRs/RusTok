use crate::{
    CanvasRect, CapabilityState, CommandCapabilityRequirement, DragSource, DragState, EditorCapability, FlyUiState,
    HitTestCandidate, PanelKind, Presentation, UiError, UiResult, ViewportState, command_for_drop,
};
use fly::{EditorCommand, ProjectHash, ValidationDiagnostic};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UiIntent {
    SetPresentation(Presentation),
    SetEditableCapabilities(CapabilityState),
    TogglePanel(PanelKind),
    ActivatePanel(PanelKind),
    SetViewport(ViewportState),
    ActivatePage {
        page_id: Option<String>,
        page_index: usize,
    },
    Select(Option<String>),
    Hover(Option<String>),
    SetSelectedOverlay(Option<CanvasRect>),
    SetHoveredOverlay(Option<CanvasRect>),
    BeginDrag(DragSource),
    UpdateHitTest(Vec<HitTestCandidate>),
    ActivateDropCandidate(Option<usize>),
    Drop,
    CancelDrag,
    CopySelection,
    CutSelection,
    PasteClipboard,
    Execute(Box<EditorCommand>),
    Undo,
    Redo,
    RequestSave,
    SaveStarted {
        request_id: String,
    },
    SaveSucceeded {
        request_id: String,
        revision: String,
        project_hash: ProjectHash,
    },
    SaveFailed {
        request_id: Option<String>,
    },
    /// The server or a peer-tab notification indicates that the document changed elsewhere.
    /// Carries expected and actual revision labels for the user-facing explanation.
    SaveConflict {
        request_id: Option<String>,
        expected: String,
        actual: String,
    },
    ReplaceDiagnostics(Vec<ValidationDiagnostic>),
}

impl UiIntent {
    /// True when dispatching the intent can change the authored document or submit it.
    /// View-state intents remain available when a peer tab has fenced this editor.
    pub fn mutates_document(&self) -> bool {
        matches!(
            self,
            Self::Drop
                | Self::CutSelection
                | Self::PasteClipboard
                | Self::Execute(_)
                | Self::Undo
                | Self::Redo
                | Self::RequestSave
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UiEffect {
    None,
    Command(Box<EditorCommand>),
    Undo,
    Redo,
    CopySelection,
    CutSelection,
    PasteClipboard,
    Persist {
        expected_hash: Option<ProjectHash>,
        command_sequence: u64,
    },
    Announce(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlyUiStateMachine {
    pub state: FlyUiState,
    #[serde(default = "CapabilityState::full")]
    editable_capabilities: CapabilityState,
}

impl FlyUiStateMachine {
    pub fn new(presentation: Presentation) -> Self {
        Self {
            state: FlyUiState::new(presentation),
            editable_capabilities: CapabilityState::full(),
        }
    }

    pub fn with_editable_capabilities(mut self, capabilities: CapabilityState) -> Self {
        self.set_editable_capabilities(capabilities);
        self
    }

    pub const fn editable_capabilities(&self) -> CapabilityState {
        self.editable_capabilities
    }

    pub fn set_editable_capabilities(&mut self, capabilities: CapabilityState) {
        self.editable_capabilities = capabilities.normalized();
        self.refresh_effective_capabilities();
    }

    pub fn dispatch(&mut self, intent: UiIntent) -> UiResult<Vec<UiEffect>> {
        let effects = match intent {
            UiIntent::SetPresentation(presentation) => {
                self.state.presentation = presentation;
                self.refresh_effective_capabilities();
                vec![UiEffect::None]
            }
            UiIntent::SetEditableCapabilities(capabilities) => {
                self.set_editable_capabilities(capabilities);
                vec![UiEffect::None]
            }
            UiIntent::TogglePanel(panel) => {
                self.state.panels.toggle(panel);
                vec![UiEffect::None]
            }
            UiIntent::ActivatePanel(panel) => {
                self.state.panels.activate(panel);
                vec![UiEffect::None]
            }
            UiIntent::SetViewport(viewport) => {
                self.state.viewport = viewport.normalized();
                vec![UiEffect::None]
            }
            UiIntent::ActivatePage {
                page_id,
                page_index,
            } => {
                self.state.page.active_page_id = page_id;
                self.state.page.active_page_index = page_index;
                self.state.set_selected_component(None);
                self.state.set_hovered_component(None);
                self.state.invalidate_document_geometry();
                vec![UiEffect::Command(Box::new(EditorCommand::Select {
                    component_id: None,
                }))]
            }
            UiIntent::Select(component_id) => {
                self.state.set_selected_component(component_id.clone());
                vec![UiEffect::Command(Box::new(EditorCommand::Select {
                    component_id,
                }))]
            }
            UiIntent::Hover(component_id) => {
                self.state.set_hovered_component(component_id);
                vec![UiEffect::None]
            }
            UiIntent::SetSelectedOverlay(rect) => {
                if rect.is_some_and(|rect| !rect.is_valid()) {
                    return Err(UiError::InvalidGeometry("selected overlay".to_string()));
                }
                self.state.overlays.selected =
                    rect.filter(|_| self.state.selection.component_id.is_some());
                self.state.overlays.resize_handles_visible =
                    self.state.overlays.selected.is_some() && self.effective_capabilities().styles;
                vec![UiEffect::None]
            }
            UiIntent::SetHoveredOverlay(rect) => {
                if rect.is_some_and(|rect| !rect.is_valid()) {
                    return Err(UiError::InvalidGeometry("hovered overlay".to_string()));
                }
                self.state.overlays.hovered =
                    rect.filter(|_| self.state.selection.hovered_component_id.is_some());
                vec![UiEffect::None]
            }
            UiIntent::BeginDrag(source) => {
                self.require_edit_capability(EditorCapability::DragDrop)?;
                self.state.drag = Some(DragState {
                    source,
                    candidates: Vec::new(),
                    active_candidate: None,
                });
                self.state.overlays.insertion = None;
                vec![UiEffect::Announce("Drag started".to_string())]
            }
            UiIntent::UpdateHitTest(candidates) => {
                if candidates.iter().any(|candidate| {
                    !candidate.rect.is_valid() || !candidate.score.is_finite()
                }) {
                    return Err(UiError::InvalidGeometry("drop candidate".to_string()));
                }
                let insertion = {
                    let drag = self.state.drag.as_mut().ok_or(UiError::NoActiveDrag)?;
                    drag.candidates = candidates;
                    drag.active_candidate =
                        drag.candidates.iter().position(|candidate| candidate.legal);
                    drag.active_candidate()
                        .filter(|candidate| candidate.legal)
                        .map(|candidate| candidate.rect)
                };
                self.state.overlays.insertion = insertion;
                vec![UiEffect::None]
            }
            UiIntent::ActivateDropCandidate(index) => {
                let insertion = {
                    let drag = self.state.drag.as_mut().ok_or(UiError::NoActiveDrag)?;
                    if let Some(index) = index {
                        let candidate = drag.candidates.get(index).ok_or_else(|| {
                            UiError::IllegalDrop(
                                "drop candidate index is out of bounds".to_string(),
                            )
                        })?;
                        candidate.validate_for_drop()?;
                    }
                    drag.active_candidate = index;
                    drag.active_candidate()
                        .filter(|candidate| candidate.legal)
                        .map(|candidate| candidate.rect)
                };
                self.state.overlays.insertion = insertion;
                vec![UiEffect::None]
            }
            UiIntent::Drop => {
                // Authorization must be revalidated at commit time, not inherited from
                // BeginDrag: restored state or changed policy must not enable a write.
                self.require_edit_capability(EditorCapability::DragDrop)?;
                // Build and validate the command before consuming the drag. A rejected
                // candidate or unsupported source must remain retryable/cancellable.
                let drag = self.state.drag.as_ref().ok_or(UiError::NoActiveDrag)?;
                let candidate = drag
                    .active_candidate()
                    .ok_or_else(|| UiError::IllegalDrop("no active drop candidate".to_string()))?;
                let command = command_for_drop(drag.source.clone(), candidate)?;
                self.state.drag = None;
                self.state.overlays.insertion = None;
                self.mark_dirty();
                vec![
                    UiEffect::Command(Box::new(command)),
                    UiEffect::Announce("Component dropped".to_string()),
                ]
            }
            UiIntent::CancelDrag => {
                self.state.drag = None;
                self.state.overlays.insertion = None;
                vec![UiEffect::Announce("Drag cancelled".to_string())]
            }
            UiIntent::CopySelection => {
                self.require_edit_capability(EditorCapability::Clipboard)?;
                if self.state.selection.component_id.is_none() {
                    return Err(UiError::CapabilityUnavailable(
                        "copy requires a selected component".to_string(),
                    ));
                }
                vec![UiEffect::CopySelection]
            }
            UiIntent::CutSelection => {
                self.require_edit_capability(EditorCapability::Clipboard)?;
                if self.state.selection.component_id.is_none() {
                    return Err(UiError::CapabilityUnavailable(
                        "cut requires a selected component".to_string(),
                    ));
                }
                self.mark_dirty();
                vec![UiEffect::CutSelection]
            }
            UiIntent::PasteClipboard => {
                self.require_edit_capability(EditorCapability::Clipboard)?;
                self.mark_dirty();
                vec![UiEffect::PasteClipboard]
            }
            UiIntent::Execute(command) => {
                let requirement = CommandCapabilityRequirement::for_command(command.as_ref());
                if !requirement.is_empty() {
                    if !self.state.presentation.is_editable() {
                        return Err(UiError::ReadOnly);
                    }
                    if let Some(capability) = requirement.first_missing(self.effective_capabilities()) {
                        return Err(UiError::CapabilityUnavailable(
                            capability.as_str().to_string(),
                        ));
                    }
                    if !matches!(command.as_ref(), EditorCommand::Batch { commands } if commands.is_empty()) {
                        self.mark_dirty();
                    }
                }
                vec![UiEffect::Command(command)]
            }
            UiIntent::Undo => {
                self.require_edit_capability(EditorCapability::History)?;
                self.mark_dirty();
                vec![UiEffect::Undo]
            }
            UiIntent::Redo => {
                self.require_edit_capability(EditorCapability::History)?;
                self.mark_dirty();
                vec![UiEffect::Redo]
            }
            UiIntent::RequestSave => {
                // Saving the working draft is an authoring action: it
                // requires the edit capability (pages:update at the
                // permission layer), not the publish capability. Real
                // publication is a separate flow with its own gate.
                self.require_edit_capability(EditorCapability::Edit)?;
                if self.state.has_blocking_diagnostics() {
                    return Err(UiError::CapabilityUnavailable(
                        "save blocked by validation diagnostics".to_string(),
                    ));
                }
                if self.state.dirty.save_conflict {
                    return Err(UiError::CapabilityUnavailable(
                        "save blocked by unresolved revision conflict".to_string(),
                    ));
                }
                if self.state.dirty.save_in_progress
                    || self.state.dirty.save_started_request_id.is_some()
                {
                    return Err(UiError::CapabilityUnavailable(
                        "save already in progress".to_string(),
                    ));
                }
                vec![UiEffect::Persist {
                    expected_hash: self.state.dirty.project_hash,
                    command_sequence: self.state.dirty.command_sequence,
                }]
            }
            UiIntent::SaveStarted { request_id } => {
                self.require_edit_capability(EditorCapability::Edit)?;
                if request_id.trim().is_empty() || request_id.len() > 128 {
                    return Err(UiError::CapabilityUnavailable(
                        "save request identity is missing or invalid".to_string(),
                    ));
                }
                if self.state.dirty.save_conflict {
                    return Err(UiError::CapabilityUnavailable(
                        "save blocked by unresolved revision conflict".to_string(),
                    ));
                }
                if self.state.dirty.save_in_progress
                    || self.state.dirty.save_started_request_id.is_some()
                {
                    return Err(UiError::CapabilityUnavailable(
                        "save already in progress".to_string(),
                    ));
                }
                self.state.dirty.save_started_request_id = Some(request_id);
                self.state.dirty.save_started_sequence =
                    Some(self.state.dirty.command_sequence);
                self.state.dirty.save_started_hash = self.state.dirty.project_hash;
                self.state.dirty.save_in_progress = true;
                self.state.dirty.save_failed = false;
                vec![UiEffect::None]
            }
            UiIntent::SaveSucceeded {
                request_id,
                revision,
                project_hash,
            } => {
                if !self.state.dirty.save_in_progress
                    || self.state.dirty.save_started_sequence.is_none()
                    || self.state.dirty.save_started_request_id.as_deref()
                        != Some(request_id.as_str())
                {
                    return Err(UiError::CapabilityUnavailable(
                        "save response does not match the active request".to_string(),
                    ));
                }
                if self.state.dirty.save_started_hash.is_some_and(|hash| hash != project_hash) {
                    return Err(UiError::CapabilityUnavailable(
                        "save response does not match the submitted project hash".to_string(),
                    ));
                }
                self.state.dirty.save_started_hash = None;
                self.state.dirty.save_started_request_id = None;
                // The server acknowledged the snapshot that existed at SaveStarted, not
                // edits made while the request was in flight. Never advertise those edits
                // as saved or replace their local hash with the older snapshot's hash.
                let newer_edits = self
                    .state
                    .dirty
                    .save_started_sequence
                    .take()
                    .is_some_and(|sequence| sequence != self.state.dirty.command_sequence);
                self.state.dirty.dirty = newer_edits;
                self.state.dirty.save_in_progress = false;
                self.state.dirty.save_failed = false;
                self.state.dirty.save_conflict = false;
                self.state.dirty.last_acknowledged_revision = Some(revision);
                if !newer_edits {
                    self.state.dirty.project_hash = Some(project_hash);
                }
                vec![UiEffect::Announce(if newer_edits {
                    "Earlier edits saved; newer changes remain unsaved".to_string()
                } else {
                    "Project saved".to_string()
                })]
            }
            UiIntent::SaveFailed { request_id } => {
                match (
                    self.state.dirty.save_started_request_id.as_deref(),
                    request_id.as_deref(),
                    self.state.dirty.save_in_progress,
                ) {
                    (Some(active), Some(received), true) if active == received => {}
                    (None, None, false) => {}
                    _ => {
                        return Err(UiError::CapabilityUnavailable(
                            "save failure does not match the active request".to_string(),
                        ));
                    }
                }
                self.state.dirty.save_started_request_id = None;
                self.state.dirty.save_started_hash = None;
                self.state.dirty.save_started_sequence = None;
                self.state.dirty.save_in_progress = false;
                self.state.dirty.save_failed = true;
                // Do not silently clear an unresolved revision conflict.
                vec![UiEffect::Announce("Project save failed".to_string())]
            }
            UiIntent::SaveConflict {
                request_id,
                expected,
                actual,
            } => {
                match (
                    self.state.dirty.save_started_request_id.as_deref(),
                    request_id.as_deref(),
                    self.state.dirty.save_in_progress,
                ) {
                    (Some(active), Some(received), true) if active == received => {}
                    (None, None, false) => {}
                    _ => {
                        return Err(UiError::CapabilityUnavailable(
                            "save conflict does not match the active request".to_string(),
                        ));
                    }
                }
                // A lost update is not a generic failure: the local changes are still
                // unsaved (`dirty` stays set) and retrying against the same revision
                // will conflict again, so the announcement tells the user to preserve their work.
                self.state.dirty.save_started_request_id = None;
                self.state.dirty.save_started_hash = None;
                self.state.dirty.save_started_sequence = None;
                self.state.dirty.dirty = true;
                self.state.dirty.save_in_progress = false;
                self.state.dirty.save_failed = true;
                self.state.dirty.save_conflict = true;
                self.refresh_effective_capabilities();
                vec![UiEffect::Announce(format!(
                    "Save conflict: this page changed in another session (expected {expected}, found {actual}). Your changes are not saved; preserve them before opening the latest version."
                ))]
            }
            UiIntent::ReplaceDiagnostics(diagnostics) => {
                self.state.set_diagnostics(diagnostics);
                vec![UiEffect::None]
            }
        };

        for effect in &effects {
            if let UiEffect::Announce(message) = effect {
                self.state.announcements.push(message.clone());
            }
        }
        Ok(effects)
    }

    fn refresh_effective_capabilities(&mut self) {
        let capabilities = if self.state.presentation.is_editable()
            && !self.state.dirty.save_conflict
        {
            self.editable_capabilities.normalized()
        } else {
            CapabilityState::read_only()
        };
        if !capabilities.drag_drop {
            self.state.drag = None;
            self.state.overlays.insertion = None;
        }
        if !capabilities.styles {
            self.state.overlays.resize_handles_visible = false;
        }
        self.state.capabilities = capabilities;
    }

    /// The public presentation state is a projection, not authority to grant
    /// capabilities above the host-evaluated ceiling. Recheck both at admission,
    /// including inconsistent restored state with edit denied and a sub-capability set.
    fn effective_capabilities(&self) -> CapabilityState {
        if !self.state.presentation.is_editable() || self.state.dirty.save_conflict {
            return CapabilityState::read_only();
        }
        self.state.capabilities.intersection(self.editable_capabilities).normalized()
    }

    fn require_edit_capability(&self, capability: EditorCapability) -> UiResult<()> {
        if !self.state.presentation.is_editable() {
            return Err(UiError::ReadOnly);
        }
        if !self.effective_capabilities().allows(capability) {
            return Err(UiError::CapabilityUnavailable(capability.as_str().to_string()));
        }
        Ok(())
    }

    /// Mirrors `RevisionState::mark_changed`: an edit after a failed save keeps `save_failed`
    /// set, because the changes are still unsaved. `SaveStarted` and `SaveSucceeded` clear it.
    fn mark_dirty(&mut self) {
        self.state.dirty.dirty = true;
        self.state.dirty.command_sequence = self.state.dirty.command_sequence.saturating_add(1);
    }
}

impl UiIntent {
    /// Constructs a command intent without exposing the enum's storage optimization.
    pub fn execute(command: EditorCommand) -> Self {
        Self::Execute(Box::new(command))
    }
}
