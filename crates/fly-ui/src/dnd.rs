use crate::{CanvasRect, UiError, UiResult};
use fly::{ComponentNode, EditorCommand};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DropPosition {
    Before,
    Inside,
    After,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HitTestCandidate {
    pub target_component_id: String,
    pub parent_component_id: Option<String>,
    pub index: usize,
    pub position: DropPosition,
    pub rect: CanvasRect,
    pub score: f32,
    pub legal: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DragSource {
    ExistingComponent { component_id: String },
    PaletteBlock { block_id: String, component: ComponentNode },
    ClipboardFragment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DragState {
    pub source: DragSource,
    pub candidates: Vec<HitTestCandidate>,
    pub active_candidate: Option<usize>,
}

impl DragState {
    pub fn active_candidate(&self) -> Option<&HitTestCandidate> {
        self.active_candidate
            .and_then(|index| self.candidates.get(index))
    }
}

pub(crate) fn command_for_drop(
    source: DragSource,
    candidate: &HitTestCandidate,
) -> UiResult<EditorCommand> {
    let parent_id = match candidate.position {
        DropPosition::Inside => Some(candidate.target_component_id.clone()),
        DropPosition::Before | DropPosition::After => candidate.parent_component_id.clone(),
    };

    match source {
        DragSource::PaletteBlock { component, .. } => Ok(EditorCommand::Insert {
            parent_id,
            index: candidate.index,
            component,
        }),
        DragSource::ExistingComponent { component_id } => Ok(EditorCommand::Move {
            component_id,
            new_parent_id: parent_id,
            index: candidate.index,
        }),
        DragSource::ClipboardFragment => Err(UiError::IllegalDrop(
            "clipboard fragments are inserted by the host fragment service".to_string(),
        )),
    }
}
