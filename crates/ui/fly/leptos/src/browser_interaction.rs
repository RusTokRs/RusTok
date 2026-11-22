#[expect(unused_imports, reason = "contracted facade re-export for wasm-client surface")]
pub use fly_web::{
    BrowserResizeSession, key_stroke_from_event, prevent_editor_shortcut_default,
    shortcut_from_event,
};
