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

    let submitting_text_val = submitting_text;

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
            <span class=move || if is_submitting() { "hidden" } else { "inline-flex items-center gap-1.5" }>
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
