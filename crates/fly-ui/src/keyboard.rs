use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ModifierState {
    pub shift: bool,
    pub alt: bool,
    pub control: bool,
    pub meta: bool,
}

impl ModifierState {
    pub const fn primary(self) -> bool {
        self.control || self.meta
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyStroke {
    pub key: String,
    pub code: Option<String>,
    pub modifiers: ModifierState,
    pub repeat: bool,
    pub editing_text: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EditorShortcut {
    Undo,
    Redo,
    Save,
    Copy,
    Cut,
    Paste,
    Duplicate,
    DeleteSelection,
    Cancel,
    MoveSelectionUp,
    MoveSelectionDown,
}

impl EditorShortcut {
    pub const fn mutates_project(self) -> bool {
        matches!(
            self,
            Self::Cut
                | Self::Paste
                | Self::Duplicate
                | Self::DeleteSelection
                | Self::MoveSelectionUp
                | Self::MoveSelectionDown
                | Self::Undo
                | Self::Redo
        )
    }

    pub const fn allowed_while_editing_text(self) -> bool {
        matches!(self, Self::Save | Self::Cancel)
    }
}

pub fn resolve_editor_shortcut(stroke: &KeyStroke) -> Option<EditorShortcut> {
    let key = normalize_key(&stroke.key);
    let primary = stroke.modifiers.primary();
    let shortcut = match key.as_str() {
        "escape" => EditorShortcut::Cancel,
        "s" if primary => EditorShortcut::Save,
        "z" if primary && stroke.modifiers.shift => EditorShortcut::Redo,
        "z" if primary => EditorShortcut::Undo,
        "y" if primary => EditorShortcut::Redo,
        "c" if primary => EditorShortcut::Copy,
        "x" if primary => EditorShortcut::Cut,
        "v" if primary => EditorShortcut::Paste,
        "d" if primary => EditorShortcut::Duplicate,
        "delete" | "backspace" if !primary => EditorShortcut::DeleteSelection,
        "arrowup" if stroke.modifiers.alt => EditorShortcut::MoveSelectionUp,
        "arrowdown" if stroke.modifiers.alt => EditorShortcut::MoveSelectionDown,
        _ => return None,
    };
    if stroke.repeat && matches!(shortcut, EditorShortcut::Save | EditorShortcut::Duplicate) {
        return None;
    }
    if stroke.editing_text && !shortcut.allowed_while_editing_text() {
        return None;
    }
    Some(shortcut)
}

fn normalize_key(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(key: &str, modifiers: ModifierState) -> KeyStroke {
        KeyStroke {
            key: key.to_string(),
            code: None,
            modifiers,
            repeat: false,
            editing_text: false,
        }
    }

    #[test]
    fn primary_modifier_supports_control_and_meta() {
        assert_eq!(
            resolve_editor_shortcut(&stroke(
                "z",
                ModifierState { control: true, ..ModifierState::default() }
            )),
            Some(EditorShortcut::Undo)
        );
        assert_eq!(
            resolve_editor_shortcut(&stroke(
                "s",
                ModifierState { meta: true, ..ModifierState::default() }
            )),
            Some(EditorShortcut::Save)
        );
    }

    #[test]
    fn text_editing_does_not_intercept_native_copy_paste() {
        let mut input = stroke(
            "c",
            ModifierState { control: true, ..ModifierState::default() },
        );
        input.editing_text = true;
        assert_eq!(resolve_editor_shortcut(&input), None);
    }

    #[test]
    fn shifted_primary_z_redoes() {
        assert_eq!(
            resolve_editor_shortcut(&stroke(
                "Z",
                ModifierState { control: true, shift: true, ..ModifierState::default() }
            )),
            Some(EditorShortcut::Redo)
        );
    }
}
