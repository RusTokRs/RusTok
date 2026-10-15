//! Layout shells and the `fly-web` re-export surface of the Leptos adapter.
//!
//! Four `#[component]` wrappers render a labelled `<section>` around their children
//! ([`FlyFullEditor`], [`FlyInlineEditor`], [`FlyPreview`], [`FlyReadOnly`]), and the module
//! re-exports `fly-web` so a Leptos host reaches the geometry, iframe-bridge and real-DOM inline
//! contracts through this crate. There is no canvas, drag & drop, panel or state wiring here:
//! canonical project state, drop legality, commands, history and editor policy stay in `fly` and
//! `fly-ui`, and the authoring surfaces that need them drive those crates directly — see
//! `crates/modules/rustok-page-builder/admin`.

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
        assert!((transform.normalized_zoom() - 1.0).abs() < f64::EPSILON);
    }
}
