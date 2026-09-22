use fly_ui::{
    CanvasRect, EditorShortcut, KeyStroke, ModifierState, ResizeHandle, ResizePolicy, ResizeResult,
    ResizeSession, resolve_editor_shortcut,
};
use wasm_bindgen::JsCast;
use web_sys::{Element, EventTarget, KeyboardEvent, PointerEvent};

pub fn key_stroke_from_event(event: &KeyboardEvent) -> KeyStroke {
    KeyStroke {
        key: event.key(),
        code: Some(event.code()),
        modifiers: ModifierState {
            shift: event.shift_key(),
            alt: event.alt_key(),
            control: event.ctrl_key(),
            meta: event.meta_key(),
        },
        repeat: event.repeat(),
        editing_text: event
            .target()
            .as_ref()
            .is_some_and(target_accepts_text_input),
    }
}

pub fn shortcut_from_event(event: &KeyboardEvent) -> Option<EditorShortcut> {
    resolve_editor_shortcut(&key_stroke_from_event(event))
}

pub fn prevent_editor_shortcut_default(event: &KeyboardEvent, shortcut: EditorShortcut) {
    if matches!(
        shortcut,
        EditorShortcut::Undo
            | EditorShortcut::Redo
            | EditorShortcut::Save
            | EditorShortcut::Copy
            | EditorShortcut::Cut
            | EditorShortcut::Paste
            | EditorShortcut::Duplicate
            | EditorShortcut::DeleteSelection
            | EditorShortcut::MoveSelectionUp
            | EditorShortcut::MoveSelectionDown
    ) {
        event.prevent_default();
    }
}

#[derive(Debug, Clone)]
pub struct BrowserResizeSession {
    pub pointer_id: i32,
    pub resize: ResizeSession,
    coordinate_scale: f64,
}

impl BrowserResizeSession {
    pub fn begin(
        component_id: impl Into<String>,
        handle: ResizeHandle,
        rect: CanvasRect,
        event: &PointerEvent,
        policy: ResizePolicy,
    ) -> Self {
        Self::begin_scaled(component_id, handle, rect, event, policy, 1.0)
    }

    pub fn begin_scaled(
        component_id: impl Into<String>,
        handle: ResizeHandle,
        rect: CanvasRect,
        event: &PointerEvent,
        policy: ResizePolicy,
        coordinate_scale: f64,
    ) -> Self {
        let coordinate_scale = normalize_scale(coordinate_scale);
        Self {
            pointer_id: event.pointer_id(),
            resize: ResizeSession {
                component_id: component_id.into(),
                handle,
                start_rect: rect,
                start_x: f64::from(event.client_x()) / coordinate_scale,
                start_y: f64::from(event.client_y()) / coordinate_scale,
                policy,
            },
            coordinate_scale,
        }
    }

    pub fn accepts(&self, event: &PointerEvent) -> bool {
        event.pointer_id() == self.pointer_id
    }

    pub fn update(&self, event: &PointerEvent) -> Option<ResizeResult> {
        self.accepts(event).then(|| {
            self.resize.update(
                f64::from(event.client_x()) / self.coordinate_scale,
                f64::from(event.client_y()) / self.coordinate_scale,
            )
        })
    }
}

fn normalize_scale(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn target_accepts_text_input(target: &EventTarget) -> bool {
    let Some(element) = target.dyn_ref::<Element>() else {
        return false;
    };
    let tag = element.tag_name().to_ascii_lowercase();
    matches!(tag.as_str(), "input" | "textarea" | "select")
        || element
            .get_attribute("contenteditable")
            .is_some_and(|value| value.is_empty() || value.eq_ignore_ascii_case("true"))
        || element
            .closest("[contenteditable='true']")
            .ok()
            .flatten()
            .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_session_tracks_pointer_identity_and_scale() {
        let resize = ResizeSession {
            component_id: "hero".to_string(),
            handle: ResizeHandle::East,
            start_rect: CanvasRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 50.0,
            },
            start_x: 100.0,
            start_y: 50.0,
            policy: ResizePolicy::default(),
        };
        let session = BrowserResizeSession {
            pointer_id: 7,
            resize,
            coordinate_scale: 0.8,
        };
        assert_eq!(session.pointer_id, 7);
        assert_eq!(session.coordinate_scale, 0.8);
    }
}
