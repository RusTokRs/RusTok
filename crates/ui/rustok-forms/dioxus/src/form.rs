//! `<Form>` — top-level form wrapper providing [`FormContext`] to descendants.

use dioxus::prelude::*;
use rustok_forms::{DirtyTracker, FormState};

use crate::context::FormContext;

/// Top-level form component.
///
/// Renders a `<form>` element, prevents default submission, provides
/// [`FormContext`] to all descendant components, and guards against
/// double-submit while the form is in the submitting state.
///
/// # Usage
///
/// ```rust,ignore
/// let form_state = use_signal(FormState::idle);
///
/// rsx! {
///     Form {
///         state: form_state,
///         on_submit: move |_| { /* handle submit */ },
///         FormField {
///             name: "title",
///             FormLabel { required: true, "Title" }
///             // ...
///         }
///     }
/// }
/// ```
#[component]
pub fn Form(
    /// Reactive form state. `<Form>` reads this to prevent double-submit
    /// and provides it as context.
    state: Signal<FormState>,
    /// Optional dirty field tracker. If provided, inputs automatically mark
    /// modified fields and `<ResetButton>` resets the tracker.
    #[props(default)]
    dirty_tracker: Option<Signal<DirtyTracker>>,
    /// Called when the form is submitted.
    on_submit: EventHandler<()>,
    /// Whether `<Form>` automatically sets `state` to `submitting` before firing `on_submit`.
    /// Defaults to `true`.
    #[props(default = true)]
    auto_submitting: bool,
    /// Optional callback fired on form reset.
    #[props(default)]
    on_reset: Option<EventHandler<()>>,
    /// HTML form method (e.g. "post", "get", "dialog").
    #[props(default)]
    method: Option<String>,
    /// HTML form action URL.
    #[props(default)]
    action: Option<String>,
    /// HTML form encoding type (e.g. "multipart/form-data").
    #[props(default)]
    enctype: Option<String>,
    /// Extra CSS classes on the `<form>` element.
    #[props(default)]
    class: Option<String>,
    /// HTML id attribute on the `<form>` element.
    #[props(default)]
    id: Option<String>,
    /// ARIA label for accessibility.
    #[props(default)]
    aria_label: Option<String>,
    /// ARIA describedby for accessibility.
    #[props(default)]
    aria_describedby: Option<String>,
    /// Child elements inside the form.
    children: Element,
) -> Element {
    let mut form_ctx = FormContext::new(state);
    if let Some(tracker) = dirty_tracker {
        form_ctx = form_ctx.with_dirty_tracker(tracker);
    }
    provide_context(form_ctx);

    let form_class = class.unwrap_or_default();

    rsx! {
        form {
            id: id,
            method: method,
            action: action,
            enctype: enctype,
            aria_label: aria_label,
            aria_describedby: aria_describedby,
            class: "{form_class}",
            novalidate: true,
            onsubmit: move |ev| {
                ev.stop_propagation();
                if state.read().is_submitting {
                    return;
                }
                if auto_submitting {
                    state.write().set_submitting();
                }
                on_submit.call(());
            },
            onreset: move |_| {
                if let Some(cb) = on_reset {
                    cb.call(());
                }
                state.write().reset();
                if let Some(mut tracker) = dirty_tracker {
                    tracker.write().reset();
                }
            },
            {children}
        }
    }
}
