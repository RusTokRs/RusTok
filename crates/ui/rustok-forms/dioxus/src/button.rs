//! Button components integrated with form submission and lifecycle state.

use dioxus::prelude::*;

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
    #[props(default)]
    variant: ButtonVariant,
    /// Size preset. Defaults to `Md`.
    #[props(default)]
    size: ButtonSize,
    /// Whether the button occupies 100% of the container width.
    #[props(default = false)]
    full_width: bool,
    /// Optional text displayed during submission (e.g. "Saving..." or "Сохранение...").
    #[props(default)]
    submitting_text: Option<String>,
    /// Additional disabled condition.
    #[props(default = false)]
    disabled: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
    /// Button children content.
    children: Element,
) -> Element {
    let form_ctx = try_use_context::<FormContext>();
    let is_submitting = form_ctx.map(|ctx| ctx.is_submitting()).unwrap_or(false);
    let is_disabled = disabled || is_submitting;

    let base = "inline-flex items-center justify-center font-semibold transition select-none disabled:cursor-not-allowed disabled:opacity-60";
    let var_cls = variant.class_names();
    let size_cls = size.class_names();
    let width_cls = if full_width { "w-full" } else { "" };
    let custom_class = class.unwrap_or_default();
    let btn_class = format!("{base} {var_cls} {size_cls} {width_cls} {custom_class}").trim().to_string();

    let has_submitting_text = submitting_text.is_some();

    rsx! {
        button {
            r#type: "submit",
            disabled: is_disabled,
            "aria-busy": if is_submitting { "true" } else { "false" },
            class: "{btn_class}",
            if is_submitting {
                span {
                    class: "w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin shrink-0"
                }
            }
            if let Some(ref txt) = submitting_text {
                if is_submitting {
                    span { "{txt}" }
                }
            }
            if !is_submitting || !has_submitting_text {
                span {
                    class: "inline-flex items-center gap-1.5",
                    {children}
                }
            }
        }
    }
}

/// Reset button that clears form values and error state.
#[component]
pub fn ResetButton(
    /// Visual style variant. Defaults to `Outline`.
    #[props(default)]
    variant: ButtonVariant,
    /// Size preset. Defaults to `Md`.
    #[props(default)]
    size: ButtonSize,
    /// Whether the button occupies 100% of the container width.
    #[props(default = false)]
    full_width: bool,
    /// Callback executed on reset.
    #[props(default)]
    on_reset: Option<EventHandler<()>>,
    /// Additional disabled condition.
    #[props(default = false)]
    disabled: bool,
    /// Extra CSS classes.
    #[props(default)]
    class: Option<String>,
    /// Button children content.
    children: Element,
) -> Element {
    let form_ctx = try_use_context::<FormContext>();
    let is_submitting = form_ctx.map(|ctx| ctx.is_submitting()).unwrap_or(false);
    let is_disabled = disabled || is_submitting;

    let base = "inline-flex items-center justify-center font-medium transition select-none disabled:cursor-not-allowed disabled:opacity-60";
    let var_cls = variant.class_names();
    let size_cls = size.class_names();
    let width_cls = if full_width { "w-full" } else { "" };
    let custom_class = class.unwrap_or_default();
    let btn_class = format!("{base} {var_cls} {size_cls} {width_cls} {custom_class}").trim().to_string();

    rsx! {
        button {
            r#type: "button",
            disabled: is_disabled,
            class: "{btn_class}",
            onclick: move |_| {
                if !is_disabled {
                    if let Some(cb) = on_reset {
                        cb.call(());
                    }
                    if let Some(mut ctx) = form_ctx {
                        ctx.state.write().reset();
                        if let Some(mut tracker) = ctx.dirty_tracker {
                            tracker.write().reset();
                        }
                    }
                }
            },
            {children}
        }
    }
}
