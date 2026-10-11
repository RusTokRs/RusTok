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
    let shortcut = shortcut_for_key(&normalize_key(&stroke.key), stroke)
        // Layout fallback. `event.key` is the *character* the active layout produces, so on a
        // non-Latin layout (Cyrillic, Greek, Hebrew, …) `Ctrl+Z` arrives as `key: "я"` and every
        // character-matched shortcut silently stops working — a Russian-language admin loses
        // undo/redo/copy/paste/save the moment the user switches layout, while the physical key is
        // still reported as `code: "KeyZ"`.
        //
        // The code is consulted only when the character did not match, which keeps a layout that
        // *does* produce a matching character on its existing behaviour: QWERTZ's `Ctrl+Z` is
        // `key: "z"`, `code: "KeyY"`, and must stay undo rather than become redo.
        .or_else(|| {
            stroke
                .code
                .as_deref()
                .and_then(physical_key)
                .and_then(|key| shortcut_for_key(&key, stroke))
        })?;
    if stroke.repeat && matches!(shortcut, EditorShortcut::Save | EditorShortcut::Duplicate) {
        return None;
    }
    if stroke.editing_text && !shortcut.allowed_while_editing_text() {
        return None;
    }
    Some(shortcut)
}

fn shortcut_for_key(key: &str, stroke: &KeyStroke) -> Option<EditorShortcut> {
    let primary = stroke.modifiers.primary();
    Some(match key {
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
    })
}

/// The shortcut token a `KeyboardEvent.code` stands for, if it names one of the keys the editor
/// binds. Only physical keys that already take part in a shortcut are translated; everything else
/// yields `None` so a code can never invent a binding the character path does not have.
fn physical_key(code: &str) -> Option<String> {
    if let Some(letter) = code.strip_prefix("Key")
        && letter.len() == 1
        && letter.is_ascii()
    {
        return Some(letter.to_ascii_lowercase());
    }
    let named = match code {
        "Escape" => "escape",
        "Delete" => "delete",
        "Backspace" => "backspace",
        "ArrowUp" => "arrowup",
        "ArrowDown" => "arrowdown",
        _ => return None,
    };
    Some(named.to_string())
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
                ModifierState {
                    control: true,
                    ..ModifierState::default()
                }
            )),
            Some(EditorShortcut::Undo)
        );
        assert_eq!(
            resolve_editor_shortcut(&stroke(
                "s",
                ModifierState {
                    meta: true,
                    ..ModifierState::default()
                }
            )),
            Some(EditorShortcut::Save)
        );
    }

    #[test]
    fn text_editing_does_not_intercept_native_copy_paste() {
        let mut input = stroke(
            "c",
            ModifierState {
                control: true,
                ..ModifierState::default()
            },
        );
        input.editing_text = true;
        assert_eq!(resolve_editor_shortcut(&input), None);
    }

    #[test]
    fn shifted_primary_z_redoes() {
        assert_eq!(
            resolve_editor_shortcut(&stroke(
                "Z",
                ModifierState {
                    control: true,
                    shift: true,
                    ..ModifierState::default()
                }
            )),
            Some(EditorShortcut::Redo)
        );
    }

    fn stroke_with_code(key: &str, code: Option<&str>, modifiers: ModifierState) -> KeyStroke {
        KeyStroke {
            key: key.to_string(),
            code: code.map(ToString::to_string),
            modifiers,
            repeat: false,
            editing_text: false,
        }
    }

    #[test]
    fn non_latin_layouts_fall_back_to_the_physical_key() {
        // Cyrillic layout: the character is `я` but the key is still KeyZ. Before this fallback
        // undo/redo/copy/paste/save were dead on any non-Latin layout.
        let control = ModifierState {
            control: true,
            ..ModifierState::default()
        };
        assert_eq!(
            resolve_editor_shortcut(&stroke_with_code("я", Some("KeyZ"), control)),
            Some(EditorShortcut::Undo)
        );
        assert_eq!(
            resolve_editor_shortcut(&stroke_with_code("ы", Some("KeyS"), control)),
            Some(EditorShortcut::Save)
        );
        assert_eq!(
            resolve_editor_shortcut(&stroke_with_code("с", Some("KeyC"), control)),
            Some(EditorShortcut::Copy)
        );
        assert_eq!(
            resolve_editor_shortcut(&stroke_with_code("м", Some("KeyV"), control)),
            Some(EditorShortcut::Paste)
        );
        assert_eq!(
            resolve_editor_shortcut(&stroke_with_code("в", Some("KeyD"), control)),
            Some(EditorShortcut::Duplicate)
        );
    }

    #[test]
    fn the_character_wins_when_the_layout_produces_a_bound_key() {
        // QWERTZ: the key labelled Z reports `key: "z"` with `code: "KeyY"`. Preferring the
        // physical key would turn Ctrl+Z into Ctrl+Y (redo), so the character must win here.
        let control_shift = ModifierState {
            control: true,
            ..ModifierState::default()
        };
        assert_eq!(
            resolve_editor_shortcut(&stroke_with_code("z", Some("KeyY"), control_shift)),
            Some(EditorShortcut::Undo)
        );
        // Dvorak: the physical KeyZ carries `;`. The character does not match, so the fallback
        // still finds undo.
        let shift = ModifierState::default();
        assert_eq!(
            resolve_editor_shortcut(&stroke_with_code(";", Some("KeyZ"), shift)),
            None
        );
        assert_eq!(
            resolve_editor_shortcut(&stroke_with_code(
                ";",
                Some("KeyZ"),
                ModifierState {
                    control: true,
                    ..ModifierState::default()
                }
            )),
            Some(EditorShortcut::Undo)
        );
    }

    #[test]
    fn physical_fallback_covers_named_keys_and_nothing_else() {
        let alt = ModifierState {
            alt: true,
            ..ModifierState::default()
        };
        assert_eq!(
            resolve_editor_shortcut(&stroke_with_code("нечто", Some("ArrowUp"), alt)),
            Some(EditorShortcut::MoveSelectionUp)
        );
        assert_eq!(
            resolve_editor_shortcut(&stroke_with_code(
                "удалить",
                Some("Delete"),
                ModifierState::default()
            )),
            Some(EditorShortcut::DeleteSelection)
        );
        // An unbound physical key cannot invent a binding: `KeyQ` is not a shortcut.
        assert_eq!(
            resolve_editor_shortcut(&stroke_with_code(
                "й",
                Some("KeyQ"),
                ModifierState {
                    control: true,
                    ..ModifierState::default()
                }
            )),
            None
        );
        // Empty or absent codes stay on the character path.
        for code in [None, Some("")] {
            assert_eq!(
                resolve_editor_shortcut(&stroke_with_code(
                    "z",
                    code,
                    ModifierState {
                        control: true,
                        ..ModifierState::default()
                    }
                )),
                Some(EditorShortcut::Undo)
            );
            assert_eq!(
                resolve_editor_shortcut(&stroke_with_code("z", code, ModifierState::default())),
                None
            );
        }
    }

    #[test]
    fn layout_fallback_does_not_bypass_the_editing_text_or_repeat_rules() {
        let control = ModifierState {
            control: true,
            ..ModifierState::default()
        };
        let mut in_text = stroke_with_code("я", Some("KeyZ"), control);
        in_text.editing_text = true;
        assert_eq!(resolve_editor_shortcut(&in_text), None);

        let mut repeated = stroke_with_code("ы", Some("KeyS"), control);
        repeated.repeat = true;
        assert_eq!(resolve_editor_shortcut(&repeated), None);

        let mut repeated_save = stroke_with_code("ы", Some("KeyS"), control);
        repeated_save.repeat = false;
        assert_eq!(
            resolve_editor_shortcut(&repeated_save),
            Some(EditorShortcut::Save)
        );
    }
}
