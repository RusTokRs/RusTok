//! Button components integrated with form submission and lifecycle state.

use leptos::prelude::*;

use crate::context::FormContext;

/// Submit button that automatically disables and shows a spinner while the form is submitting.
#[component]
pub fn SubmitButton(
    /// Optional text displayed during submission (e.g. "Saving..." or "Сохранение...").
    #[prop(optional, into)]
    submitting_text: Option<String>,
    /// Additional disabled condition.
    #[prop(optional, into)]
    disabled: Option<Signal<bool>>,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
    children: Children,
) -> impl IntoView {
    let form_ctx = use_context::<FormContext>();

    let is_submitting = move || {
        form_ctx.as_ref().map(|ctx| ctx.state.get().is_submitting).unwrap_or(false)
    };

    let is_disabled = move || {
        if let Some(sig) = disabled {
            sig.get() || is_submitting()
        } else {
            is_submitting()
        }
    };

    let base_class = move || {
        let base = "inline-flex items-center justify-center gap-2 h-9 px-4 rounded-xl bg-primary text-primary-foreground text-xs font-semibold hover:bg-primary/90 transition shadow-sm disabled:cursor-not-allowed disabled:opacity-60";
        if class.is_empty() {
            base.to_string()
        } else {
            format!("{base} {class}")
        }
    };

    let has_submitting_text = submitting_text.is_some();
    let submitting_text_val = submitting_text;

    let children_class = move || {
        if is_submitting() && has_submitting_text {
            "hidden"
        } else {
            "inline-flex items-center gap-1.5"
        }
    };

    view! {
        <button
            type="submit"
            disabled=is_disabled
            class=base_class
        >
            <Show when=move || is_submitting()>
                <span class="w-3.5 h-3.5 border-2 border-primary-foreground/30 border-t-primary-foreground rounded-full animate-spin" />
            </Show>
            {if let Some(txt) = submitting_text_val {
                view! {
                    <span class=move || if is_submitting() { "inline" } else { "hidden" }>
                        {txt.clone()}
                    </span>
                }.into_any()
            } else {
                ().into_any()
            }}
            <span class=children_class>
                {children()}
            </span>
        </button>
    }
}

/// Reset button that clears form values and error state.
#[component]
pub fn ResetButton(
    /// Callback executed on reset.
    #[prop(optional, into)]
    on_reset: Option<Callback<()>>,
    /// Extra CSS classes.
    #[prop(optional, into)]
    class: String,
    children: Children,
) -> impl IntoView {
    let form_ctx = use_context::<FormContext>();

    let is_submitting = move || {
        form_ctx.as_ref().map(|ctx| ctx.state.get().is_submitting).unwrap_or(false)
    };

    let base_class = move || {
        let base = "inline-flex items-center justify-center gap-1.5 h-9 px-3.5 rounded-xl border border-border bg-background text-xs font-medium text-foreground hover:bg-accent transition disabled:cursor-not-allowed disabled:opacity-60";
        if class.is_empty() {
            base.to_string()
        } else {
            format!("{base} {class}")
        }
    };

    let on_click = move |_| {
        if !is_submitting() {
            if let Some(cb) = on_reset {
                cb.run(());
            }
        }
    };

    view! {
        <button
            type="button"
            disabled=is_submitting
            on:click=on_click
            class=base_class
        >
            {children()}
        </button>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_forms::FormState;

    #[test]
    fn test_submit_button_idle_renders_children() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext { state: state_signal };

        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <SubmitButton submitting_text="Saving...">"Save Changes"</SubmitButton>
            </div>
        }
        .to_html();

        assert!(html.contains("type=\"submit\""));
        assert!(html.contains("Save Changes"));
        // Spinner should not be present in idle state
        assert!(!html.contains("animate-spin"));
    }

    #[test]
    fn test_submit_button_submitting_shows_spinner_and_disabled() {
        let state = FormState::submitting();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext { state: state_signal };

        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <SubmitButton submitting_text="Saving...">"Save Changes"</SubmitButton>
            </div>
        }
        .to_html();

        assert!(html.contains("disabled"));
        assert!(html.contains("animate-spin"));
        assert!(html.contains("Saving..."));
    }

    #[test]
    fn test_reset_button_renders_type_button() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext { state: state_signal };

        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <ResetButton>"Cancel"</ResetButton>
            </div>
        }
        .to_html();

        assert!(html.contains("type=\"button\""));
        assert!(html.contains("Cancel"));
    }
}
