use crate::DragState;
use fly::{ProjectHash, ValidationDiagnostic, ValidationSeverity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Presentation {
    Full,
    Inline,
    Preview,
    ReadOnly,
}

impl Presentation {
    pub const fn is_editable(self) -> bool {
        matches!(self, Self::Full | Self::Inline)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Inline => "inline",
            Self::Preview => "preview",
            Self::ReadOnly => "read_only",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PanelKind {
    Palette,
    Layers,
    Traits,
    Styles,
    Assets,
    History,
    Diagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelState {
    pub open: BTreeSet<PanelKind>,
    pub active: Option<PanelKind>,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            open: BTreeSet::from([PanelKind::Palette, PanelKind::Layers]),
            active: Some(PanelKind::Palette),
        }
    }
}

impl PanelState {
    pub fn toggle(&mut self, panel: PanelKind) {
        if !self.open.remove(&panel) {
            self.open.insert(panel);
            self.active = Some(panel);
        } else if self.active == Some(panel) {
            self.active = self.open.iter().next_back().copied();
        }
    }

    pub fn activate(&mut self, panel: PanelKind) {
        self.open.insert(panel);
        self.active = Some(panel);
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ViewportState {
    pub width: u32,
    pub height: u32,
    pub zoom: f32,
    pub scroll_x: f64,
    pub scroll_y: f64,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            zoom: 1.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageNavigationState {
    pub active_page_id: Option<String>,
    pub active_page_index: usize,
}

impl Default for PageNavigationState {
    fn default() -> Self {
        Self {
            active_page_id: None,
            active_page_index: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SelectionState {
    pub component_id: Option<String>,
    pub hovered_component_id: Option<String>,
    pub property_editor_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CanvasRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl CanvasRect {
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x
            && y >= self.y
            && x <= self.x + self.width
            && y <= self.y + self.height
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OverlayState {
    pub selected: Option<CanvasRect>,
    pub hovered: Option<CanvasRect>,
    pub insertion: Option<CanvasRect>,
    pub resize_handles_visible: bool,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            selected: None,
            hovered: None,
            insertion: None,
            resize_handles_visible: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityState {
    pub edit: bool,
    pub drag_drop: bool,
    pub properties: bool,
    pub styles: bool,
    pub assets: bool,
    pub clipboard: bool,
    pub history: bool,
    pub publish: bool,
}

impl CapabilityState {
    pub const fn full() -> Self {
        Self {
            edit: true,
            drag_drop: true,
            properties: true,
            styles: true,
            assets: true,
            clipboard: true,
            history: true,
            publish: true,
        }
    }

    pub const fn read_only() -> Self {
        Self {
            edit: false,
            drag_drop: false,
            properties: false,
            styles: false,
            assets: false,
            clipboard: false,
            history: false,
            publish: false,
        }
    }
}

impl Default for CapabilityState {
    fn default() -> Self {
        Self::full()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorPolicy {
    pub allow_raw_html: bool,
    pub allow_component_scripts: bool,
    pub allow_external_urls: bool,
    pub maximum_history_entries: usize,
    pub maximum_overlay_count: usize,
}

impl Default for EditorPolicy {
    fn default() -> Self {
        Self {
            allow_raw_html: false,
            allow_component_scripts: false,
            allow_external_urls: true,
            maximum_history_entries: 100,
            maximum_overlay_count: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirtyState {
    pub dirty: bool,
    pub command_sequence: u64,
    pub last_acknowledged_revision: Option<String>,
    pub project_hash: Option<ProjectHash>,
    pub save_in_progress: bool,
    pub save_failed: bool,
}

impl Default for DirtyState {
    fn default() -> Self {
        Self {
            dirty: false,
            command_sequence: 0,
            last_acknowledged_revision: None,
            project_hash: None,
            save_in_progress: false,
            save_failed: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlyUiState {
    pub presentation: Presentation,
    pub panels: PanelState,
    pub viewport: ViewportState,
    pub page: PageNavigationState,
    pub selection: SelectionState,
    pub overlays: OverlayState,
    pub drag: Option<DragState>,
    pub capabilities: CapabilityState,
    pub policy: EditorPolicy,
    pub dirty: DirtyState,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub announcements: Vec<String>,
}

impl FlyUiState {
    pub fn new(presentation: Presentation) -> Self {
        let capabilities = if presentation.is_editable() {
            CapabilityState::full()
        } else {
            CapabilityState::read_only()
        };
        Self {
            presentation,
            panels: PanelState::default(),
            viewport: ViewportState::default(),
            page: PageNavigationState::default(),
            selection: SelectionState::default(),
            overlays: OverlayState::default(),
            drag: None,
            capabilities,
            policy: EditorPolicy::default(),
            dirty: DirtyState::default(),
            diagnostics: Vec::new(),
            announcements: Vec::new(),
        }
    }

    pub fn effective_editable(&self) -> bool {
        self.presentation.is_editable() && self.capabilities.edit
    }

    pub fn set_diagnostics(&mut self, diagnostics: Vec<ValidationDiagnostic>) {
        self.diagnostics = diagnostics;
    }

    pub fn has_blocking_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
    }
}
