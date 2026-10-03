//! Leptos/browser adapter foundation for Fly.
//!
//! This crate owns framework and browser concerns only. Canonical project state, drop legality,
//! commands, history, and editor policy remain in `fly` and `fly-ui`.
//! Browser math, iframe bridge, and geometry contracts are provided by `fly-web`.

// No hand-written unsafe in this crate. `deny` rather than `forbid` because framework
// proc-macros (`#[component]`, wasm-bindgen glue) may expand to generated unsafe guarded by their
// own `#[allow(unsafe_code)]`, which `forbid` would reject.
#![deny(unsafe_code)]

pub use fly_web::*;
use leptos::prelude::*;

#[component]
pub fn FlyFullEditor(children: Children) -> impl IntoView {
    view! {
        <section class="fly-editor fly-editor--full" role="application" aria-label="Fly full editor">
            {children()}
        </section>
    }
}

#[component]
pub fn FlyInlineEditor(children: Children) -> impl IntoView {
    view! {
        <section class="fly-editor fly-editor--inline" role="application" aria-label="Fly inline editor">
            {children()}
        </section>
    }
}

#[component]
pub fn FlyPreview(children: Children) -> impl IntoView {
    view! {
        <section class="fly-editor fly-editor--preview" aria-label="Fly preview">
            {children()}
        </section>
    }
}

#[component]
pub fn FlyReadOnly(children: Children) -> impl IntoView {
    view! {
        <section class="fly-editor fly-editor--read-only" aria-label="Fly read-only view">
            {children()}
        </section>
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
