//! Dioxus 0.6 adapter foundation for Fly visual editors.
//!
//! This crate owns Dioxus component shells, DOM element rendering, and event bindings.
//! Canonical project state, drop legality, commands, history, and editor policy remain in `fly` and `fly-ui`.
//! Browser geometry, hit-testing, and iframe bridge contracts are provided by `fly-web`.

// No hand-written unsafe in this crate. `deny` rather than `forbid` because framework
// proc-macros (`#[component]`, wasm-bindgen glue) may expand to generated unsafe guarded by their
// own `#[allow(unsafe_code)]`, which `forbid` would reject.
#![deny(unsafe_code)]

use dioxus::prelude::*;
pub use fly_web::*;

#[component]
pub fn FlyFullEditor(children: Element) -> Element {
    rsx! {
        section {
            class: "fly-editor fly-editor--full",
            role: "application",
            aria_label: "Fly full editor",
            {children}
        }
    }
}

#[component]
pub fn FlyInlineEditor(children: Element) -> Element {
    rsx! {
        section {
            class: "fly-editor fly-editor--inline",
            role: "application",
            aria_label: "Fly inline editor",
            {children}
        }
    }
}

#[component]
pub fn FlyPreview(children: Element) -> Element {
    rsx! {
        section {
            class: "fly-editor fly-editor--preview",
            aria_label: "Fly preview",
            {children}
        }
    }
}

#[component]
pub fn FlyReadOnly(children: Element) -> Element {
    rsx! {
        section {
            class: "fly-editor fly-editor--read-only",
            aria_label: "Fly read-only view",
            {children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reexported_types_are_accessible() {
        let _point = BrowserPoint { x: 0.0, y: 0.0 };
        let _rect = BrowserRect {
            left: 0.0,
            top: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let transform = CoordinateTransform::default();
        assert_eq!(transform.normalized_zoom(), 1.0);
    }
}
