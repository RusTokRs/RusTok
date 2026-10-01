use crate::{
    CanvasRect, CapabilityState, CommandCapabilityRequirement, DragSource, DragState, FlyUiState,
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
    SaveStarted,
    SaveSucceeded {
        revision: String,
        project_hash: ProjectHash,
    },
    SaveFailed,
    ReplaceDiagnostics(Vec<ValidationDiagnostic>),
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
                self.state.viewport = viewport;
                vec![UiEffect::None]
            }
            UiIntent::ActivatePage {
                page_id,
                page_index,
            } => {
                self.state.page.active_page_id = page_id;
                self.state.page.active_page_index = page_index;
                self.state.selection.component_id = None;
                self.state.selection.hovered_component_id = None;
                self.state.selection.property_editor_id = None;
                self.state.overlays.selected = None;
                self.state.overlays.hovered = None;
                self.state.overlays.insertion = None;
                self.state.overlays.resize_handles_visible = false;
                self.state.drag = None;
                vec![UiEffect::Command(Box::new(EditorCommand::Select {
                    component_id: None,
                }))]
            }
            UiIntent::Select(component_id) => {
                self.state.selection.component_id = component_id.clone();
                vec![UiEffect::Command(Box::new(EditorCommand::Select {
                    component_id,
                }))]
            }
            UiIntent::Hover(component_id) => {
                self.state.selection.hovered_component_id = component_id;
                vec![UiEffect::None]
            }
            UiIntent::SetSelectedOverlay(rect) => {
                self.state.overlays.selected = rect;
                self.state.overlays.resize_handles_visible =
                    rect.is_some() && self.state.effective_editable();
                vec![UiEffect::None]
            }
            UiIntent::SetHoveredOverlay(rect) => {
                self.state.overlays.hovered = rect;
                vec![UiEffect::None]
            }
            UiIntent::BeginDrag(source) => {
                self.require_edit_capability("drag_drop", self.state.capabilities.drag_drop)?;
                self.state.drag = Some(DragState {
                    source,
                    candidates: Vec::new(),
                    active_candidate: None,
                });
                vec![UiEffect::Announce("Drag started".to_string())]
            }
            UiIntent::UpdateHitTest(candidates) => {
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
                        if !candidate.legal {
                            return Err(UiError::IllegalDrop(
                                candidate
                                    .reason
                                    .clone()
                                    .unwrap_or_else(|| "candidate rejected by policy".to_string()),
                            ));
                        }
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
                let drag = self.state.drag.take().ok_or(UiError::NoActiveDrag)?;
                let candidate = drag
                    .active_candidate()
                    .cloned()
                    .ok_or_else(|| UiError::IllegalDrop("no active drop candidate".to_string()))?;
                if !candidate.legal {
                    return Err(UiError::IllegalDrop(
                        candidate
                            .reason
                            .clone()
                            .unwrap_or_else(|| "candidate rejected by policy".to_string()),
                    ));
                }
                self.state.overlays.insertion = None;
                let command = command_for_drop(drag.source, &candidate)?;
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
                self.require_edit_capability("clipboard", self.state.capabilities.clipboard)?;
                if self.state.selection.component_id.is_none() {
                    return Err(UiError::CapabilityUnavailable(
                        "copy requires a selected component".to_string(),
                    ));
                }
                vec![UiEffect::CopySelection]
            }
            UiIntent::CutSelection => {
                self.require_edit_capability("clipboard", self.state.capabilities.clipboard)?;
                if self.state.selection.component_id.is_none() {
                    return Err(UiError::CapabilityUnavailable(
                        "cut requires a selected component".to_string(),
                    ));
                }
                self.mark_dirty();
                vec![UiEffect::CutSelection]
            }
            UiIntent::PasteClipboard => {
                self.require_edit_capability("clipboard", self.state.capabilities.clipboard)?;
                self.mark_dirty();
                vec![UiEffect::PasteClipboard]
            }
            UiIntent::Execute(command) => {
                let requirement = CommandCapabilityRequirement::for_command(command.as_ref());
                if !requirement.is_empty() {
                    if !self.state.presentation.is_editable() {
                        return Err(UiError::ReadOnly);
                    }
                    if let Some(capability) = requirement.first_missing(self.state.capabilities) {
                        return Err(UiError::CapabilityUnavailable(
                            capability.as_str().to_string(),
                        ));
                    }
                    self.mark_dirty();
                }
                vec![UiEffect::Command(command)]
            }
            UiIntent::Undo => {
                self.require_edit_capability("history", self.state.capabilities.history)?;
                self.mark_dirty();
                vec![UiEffect::Undo]
            }
            UiIntent::Redo => {
                self.require_edit_capability("history", self.state.capabilities.history)?;
                self.mark_dirty();
                vec![UiEffect::Redo]
            }
            UiIntent::RequestSave => {
                self.require_edit_capability("publish", self.state.capabilities.publish)?;
                if self.state.has_blocking_diagnostics() {
                    return Err(UiError::CapabilityUnavailable(
                        "publish blocked by validation diagnostics".to_string(),
                    ));
                }
                vec![UiEffect::Persist {
                    expected_hash: self.state.dirty.project_hash,
                    command_sequence: self.state.dirty.command_sequence,
                }]
            }
            UiIntent::SaveStarted => {
                self.state.dirty.save_in_progress = true;
                self.state.dirty.save_failed = false;
                vec![UiEffect::None]
            }
            UiIntent::SaveSucceeded {
                revision,
                project_hash,
            } => {
                self.state.dirty.dirty = false;
                self.state.dirty.save_in_progress = false;
                self.state.dirty.save_failed = false;
                self.state.dirty.last_acknowledged_revision = Some(revision);
                self.state.dirty.project_hash = Some(project_hash);
                vec![UiEffect::Announce("Project saved".to_string())]
            }
            UiIntent::SaveFailed => {
                self.state.dirty.save_in_progress = false;
                self.state.dirty.save_failed = true;
                vec![UiEffect::Announce("Project save failed".to_string())]
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
        let capabilities = if self.state.presentation.is_editable() {
            self.editable_capabilities
        } else {
            CapabilityState::read_only()
        };
        if !capabilities.drag_drop {
            self.state.drag = None;
            self.state.overlays.insertion = None;
        }
        if !capabilities.edit {
            self.state.overlays.resize_handles_visible = false;
        }
        self.state.capabilities = capabilities;
    }

    fn require_edit_capability(&self, name: &str, enabled: bool) -> UiResult<()> {
        if !self.state.presentation.is_editable() {
            return Err(UiError::ReadOnly);
        }
        if !enabled {
            return Err(UiError::CapabilityUnavailable(name.to_string()));
        }
        Ok(())
    }

    fn mark_dirty(&mut self) {
        self.state.dirty.dirty = true;
        self.state.dirty.command_sequence = self.state.dirty.command_sequence.saturating_add(1);
        self.state.dirty.save_failed = false;
    }
}

impl UiIntent {
    /// Constructs a command intent without exposing the enum's storage optimization.
    pub fn execute(command: EditorCommand) -> Self {
        Self::Execute(Box::new(command))
    }
}
