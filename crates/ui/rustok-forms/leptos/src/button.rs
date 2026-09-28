//! Button components integrated with form submission and lifecycle state.

use leptos::prelude::*;

use crate::context::FormContext;

/// Button visual style variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Default high-contrast primary action button.
    #[default]
    Primary,
    /// Subtle secondary action button.
    Secondary,
    /// Destructive action button for irreversible operations (red).
    Destructive,
    /// Outlined button with bordered border.
    Outline,
    /// Transparent ghost button with hover background.
    Ghost,
}

impl ButtonVariant {
    /// Return Tailwind CSS class names corresponding to this variant.
    pub fn class_names(&self) -> &'static str {
        match self {
            Self::Primary => "bg-primary text-primary-foreground hover:bg-primary/90 shadow-sm",
            Self::Secondary => "bg-secondary text-secondary-foreground hover:bg-secondary/80",
            Self::Destructive => "bg-destructive text-destructive-foreground hover:bg-destructive/90 shadow-sm",
            Self::Outline => "border border-border bg-background text-foreground hover:bg-accent hover:text-accent-foreground",
            Self::Ghost => "text-foreground hover:bg-accent hover:text-accent-foreground",
        }
    }
}

/// Button size preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ButtonSize {
    /// Small compact size (height 8, text-xs).
    Sm,
    /// Medium standard size (height 9, text-xs).
    #[default]
    Md,
    /// Large prominent size (height 10, text-sm).
    Lg,
}

impl ButtonSize {
    /// Return Tailwind CSS class names corresponding to this size preset.
    pub fn class_names(&self) -> &'static str {
        match self {
            Self::Sm => "h-8 px-3 text-xs rounded-lg gap-1.5",
            Self::Md => "h-9 px-4 text-xs rounded-xl gap-2",
            Self::Lg => "h-10 px-5 text-sm rounded-xl gap-2.5",
        }
    }
}

/// Submit button that automatically disables and shows a spinner while the form is submitting.
#[component]
pub fn SubmitButton(
    /// Visual style variant. Defaults to `Primary`.
    #[prop(default = ButtonVariant::Primary)]
    variant: ButtonVariant,
    /// Size preset. Defaults to `Md`.
    #[prop(default = ButtonSize::Md)]
    size: ButtonSize,
    /// Whether the button occupies 100% of the container width.
    #[prop(optional)]
    full_width: bool,
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
        let base = "inline-flex items-center justify-center font-semibold transition select-none disabled:cursor-not-allowed disabled:opacity-60";
        let var_cls = variant.class_names();
        let size_cls = size.class_names();
        let width_cls = if full_width { "w-full" } else { "" };
        if class.is_empty() {
            format!("{base} {var_cls} {size_cls} {width_cls}")
        } else {
            format!("{base} {var_cls} {size_cls} {width_cls} {class}")
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
            aria-busy=move || if is_submitting() { "true" } else { "false" }
            class=base_class
        >
            <Show when=move || is_submitting()>
                <span class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin shrink-0" />
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
    /// Visual style variant. Defaults to `Outline`.
    #[prop(default = ButtonVariant::Outline)]
    variant: ButtonVariant,
    /// Size preset. Defaults to `Md`.
    #[prop(default = ButtonSize::Md)]
    size: ButtonSize,
    /// Whether the button occupies 100% of the container width.
    #[prop(optional)]
    full_width: bool,
    /// Callback executed on reset.
    #[prop(optional, into)]
    on_reset: Option<Callback<()>>,
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
        let base = "inline-flex items-center justify-center font-medium transition select-none disabled:cursor-not-allowed disabled:opacity-60";
        let var_cls = variant.class_names();
        let size_cls = size.class_names();
        let width_cls = if full_width { "w-full" } else { "" };
        if class.is_empty() {
            format!("{base} {var_cls} {size_cls} {width_cls}")
        } else {
            format!("{base} {var_cls} {size_cls} {width_cls} {class}")
        }
    };

    let on_click = move |_| {
        if !is_disabled() {
            if let Some(cb) = on_reset {
                cb.run(());
            }
            if let Some(ctx) = form_ctx {
                if let Some(rw) = ctx.rw_state {
                    rw.update(|s| s.reset());
                }
                if let Some(tracker) = ctx.dirty_tracker {
                    tracker.update(|t| t.reset());
                }
            }
        }
    };

    view! {
        <button
            type="button"
            disabled=is_disabled
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
        let form_ctx = FormContext::new(state_signal);

        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <SubmitButton submitting_text="Saving...">"Save Changes"</SubmitButton>
            </div>
        }
        .to_html();

        assert!(html.contains("type=\"submit\""));
        assert!(html.contains("Save Changes"));
        assert!(html.contains("aria-busy=\"false\""));
        // Spinner should not be present in idle state
        assert!(!html.contains("animate-spin"));
    }

    #[test]
    fn test_submit_button_submitting_shows_spinner_and_disabled() {
        let state = FormState::submitting();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext::new(state_signal);

        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <SubmitButton submitting_text="Saving...">"Save Changes"</SubmitButton>
            </div>
        }
        .to_html();

        assert!(html.contains("disabled"));
        assert!(html.contains("aria-busy=\"true\""));
        assert!(html.contains("animate-spin"));
        assert!(html.contains("Saving..."));
    }

    #[test]
    fn test_reset_button_renders_type_button() {
        let state = FormState::idle();
        let state_signal = Signal::derive(move || state.clone());
        let form_ctx = FormContext::new(state_signal);

        let html = view! {
            <div>
                {provide_context(form_ctx)}
                <ResetButton variant=ButtonVariant::Destructive full_width=true>"Cancel"</ResetButton>
            </div>
        }
        .to_html();

        assert!(html.contains("type=\"button\""));
        assert!(html.contains("Cancel"));
        assert!(html.contains("bg-destructive"));
        assert!(html.contains("w-full"));
    }
}
