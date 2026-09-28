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
    /// Called when the form is submitted. The form state is set to
    /// `submitting` before this callback fires. The callback must
    /// eventually call `set_submitted_success` or `set_submitted_failure`.
    #[prop(into)]
    on_submit: Callback<()>,
    /// Extra CSS classes on the `<form>` element.
    #[prop(optional, into)]
    class: String,
    children: Children,
) -> impl IntoView {
    let read_state: Signal<FormState> = state.into();
    provide_context(FormContext { state: read_state });

    let handle_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if state.get_untracked().is_submitting {
            return;
        }
        state.update(|s| s.set_submitting());
        on_submit.run(());
    };

    view! {
        <form
            on:submit=handle_submit
            class=class
            novalidate=true
        >
            {children()}
        </form>
    }
}
