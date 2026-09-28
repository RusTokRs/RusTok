//! `<Form>` — top-level form wrapper providing [`FormContext`] to descendants.

use leptos::prelude::*;
use rustok_forms::FormState;

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
/// let form_state = RwSignal::new(FormState::idle());
///
/// view! {
///     <Form state=form_state on_submit=move |_| { /* handle */ }>
///         <FormField name="title">
///             // ...
///         </FormField>
///     </Form>
/// }
/// ```
#[component]
pub fn Form(
    /// Reactive form state. `<Form>` reads this to prevent double-submit
    /// and provides it as context. The caller owns write access.
    state: RwSignal<FormState>,
    /// Called when the form is submitted.
    #[prop(into)]
    on_submit: Callback<()>,
    /// Whether `<Form>` automatically sets `state` to `submitting` before firing `on_submit`.
    /// Defaults to `true`.
    #[prop(default = true)]
    auto_submitting: bool,
    /// Optional callback fired on form reset.
    #[prop(optional, into)]
    on_reset: Option<Callback<()>>,
    /// HTML form method (e.g. "post", "get", "dialog").
    #[prop(optional, into)]
    method: Option<String>,
    /// HTML form action URL.
    #[prop(optional, into)]
    action: Option<String>,
    /// HTML form encoding type (e.g. "multipart/form-data").
    #[prop(optional, into)]
    enctype: Option<String>,
    /// Extra CSS classes on the `<form>` element.
    #[prop(optional, into)]
    class: String,
    /// HTML id attribute on the `<form>` element.
    #[prop(optional, into)]
    id: Option<String>,
    /// ARIA label for accessibility.
    #[prop(optional, into)]
    aria_label: Option<String>,
    /// ARIA describedby for accessibility.
    #[prop(optional, into)]
    aria_describedby: Option<String>,
    children: Children,
) -> impl IntoView {
    let read_state: Signal<FormState> = state.into();
    provide_context(FormContext { state: read_state });

    let handle_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if state.get_untracked().is_submitting {
            return;
        }
        if auto_submitting {
            state.update(|s| s.set_submitting());
        }
        on_submit.run(());
    };

    let handle_reset = move |ev: leptos::ev::Event| {
        ev.prevent_default();
        if let Some(cb) = on_reset {
            cb.run(());
        }
        state.update(|s| s.reset());
    };

    view! {
        <form
            id=id
            method=method
            action=action
            enctype=enctype
            aria-label=aria_label
            aria-describedby=aria_describedby
            on:submit=handle_submit
            on:reset=handle_reset
            class=class
            novalidate=true
        >
            {children()}
        </form>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_form_renders_novalidate_and_id() {
        let state = RwSignal::new(FormState::idle());
        let html = view! {
            <Form
                state=state
                on_submit=Callback::new(|_| ())
                id="test-form"
                class="space-y-4"
                method="post"
                action="/api/submit"
                aria_label="User Registration"
            >
                <input type="text" name="name" />
            </Form>
        }
        .to_html();

        assert!(html.contains("id=\"test-form\""));
        assert!(html.contains("novalidate"));
        assert!(html.contains("class=\"space-y-4\""));
        assert!(html.contains("name=\"name\""));
        assert!(html.contains("method=\"post\""));
        assert!(html.contains("action=\"/api/submit\""));
        assert!(html.contains("aria-label=\"User Registration\""));
    }
}
