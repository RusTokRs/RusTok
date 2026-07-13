use crate::editor::AdminEditorRuntime;
use fly_ui::{EditorShortcut, UiIntent};

pub fn dispatch_shortcut(runtime: &AdminEditorRuntime, shortcut: EditorShortcut) {
    match shortcut {
        EditorShortcut::Undo => runtime.dispatch(UiIntent::Undo),
        EditorShortcut::Redo => runtime.dispatch(UiIntent::Redo),
        EditorShortcut::Save => runtime.dispatch(UiIntent::RequestSave),
        EditorShortcut::Copy => runtime.dispatch(UiIntent::CopySelection),
        EditorShortcut::Cut => runtime.dispatch(UiIntent::CutSelection),
        EditorShortcut::Paste => runtime.dispatch(UiIntent::PasteClipboard),
        EditorShortcut::Duplicate => {
            runtime.dispatch(UiIntent::CopySelection);
            if runtime.last_error.get_untracked().is_none() {
                runtime.dispatch(UiIntent::PasteClipboard);
            }
        }
        EditorShortcut::DeleteSelection => {
            let intent = runtime
                .controller
                .with(|controller| controller.remove_selected_intent());
            runtime.dispatch_result(intent);
        }
        EditorShortcut::Cancel => {
            if runtime
                .controller
                .with(|controller| controller.ui().state.drag.is_some())
            {
                runtime.dispatch(UiIntent::CancelDrag);
            }
        }
        EditorShortcut::MoveSelectionUp => {
            let intent = runtime
                .controller
                .with(|controller| controller.move_selected_up_intent());
            runtime.dispatch_result(intent);
        }
        EditorShortcut::MoveSelectionDown => {
            let intent = runtime
                .controller
                .with(|controller| controller.move_selected_down_intent());
            runtime.dispatch_result(intent);
        }
    }
}

use leptos::prelude::GetUntracked;
