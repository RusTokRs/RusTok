/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Deterministic Tailwind/shadcn CSS class resolvers for all UI component variants, sizes, and states.
//!
//! Every resolver returns a plain, space-separated class list. The resolvers
//! never inspect the environment, never touch the DOM, and never read a theme
//! at runtime, so the same input always produces the same output on the server,
//! in the browser, and in tests.
//!
//! Caller-provided `custom` classes are appended to the resolved classes. They
//! are intended for additions (spacing, width, layout), not for overriding an
//! already-resolved utility: Tailwind resolves conflicting utilities by
//! stylesheet order, not by the order inside the `class` attribute.

use crate::tokens::{
    DISABLED_CONTROL_CLASSES, DISABLED_INPUT_CLASSES, FOCUS_RING_CLASSES, INPUT_FOCUS_RING_CLASSES,
    TRANSITION_COLORS_CLASSES, radius, shadow,
};
use crate::types::{
    AlertVariant, AvatarSize, BadgeVariant, ButtonVariant, CardVariant, Orientation, Size,
    SkeletonVariant, SwitchSize,
};

/// Joins class fragments into a single space-separated class list.
///
/// Fragments are trimmed, empty fragments are skipped, and the result never
/// contains duplicated or trailing separators.
///
/// ```
/// use rustok_ui::merge_classes;
///
/// assert_eq!(merge_classes(&["a", "", "  ", "b"]), "a b");
/// assert_eq!(merge_classes(&["a ", " b"]), "a b");
/// assert_eq!(merge_classes(&[]), "");
/// ```
#[must_use = "merging class fragments has no effect unless the result is applied to an element"]
pub fn merge_classes(parts: &[&str]) -> String {
    let mut merged = String::new();

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if !merged.is_empty() {
            merged.push(' ');
        }
        merged.push_str(part);
    }

    merged
}

/// Generates a CSS class string for a button.
///
/// The disabled and loading states are expressed by the component through the
/// native `disabled` attribute and the `disabled:` utilities below, so they do
/// not change the returned class list.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn button_classes(variant: ButtonVariant, size: Size, custom: Option<&str>) -> String {
    let size_cls = match size {
        Size::Xs => "h-7 px-2.5 text-xs gap-1",
        Size::Sm => "h-8 px-3 text-xs gap-1.5",
        Size::Md => "h-9 px-4 py-2 text-sm gap-2",
        Size::Lg => "h-10 px-8 text-sm gap-2",
        Size::Xl => "h-12 px-10 text-base gap-2.5",
        Size::Icon => "h-9 w-9 p-0",
    };

    let variant_cls = match variant {
        ButtonVariant::Default => "bg-primary text-primary-foreground hover:bg-primary/90",
        ButtonVariant::Destructive => {
            "bg-destructive text-destructive-foreground hover:bg-destructive/90"
        }
        ButtonVariant::Outline => {
            "border border-input bg-background hover:bg-accent hover:text-accent-foreground"
        }
        ButtonVariant::Secondary => "bg-secondary text-secondary-foreground hover:bg-secondary/80",
        ButtonVariant::Ghost => "hover:bg-accent hover:text-accent-foreground",
        ButtonVariant::Link => "text-primary underline-offset-4 hover:underline",
    };

    let elevation = match variant {
        ButtonVariant::Default
        | ButtonVariant::Destructive
        | ButtonVariant::Outline
        | ButtonVariant::Secondary => shadow::XS,
        ButtonVariant::Ghost | ButtonVariant::Link => shadow::NONE,
    };

    merge_classes(&[
        "inline-flex items-center justify-center whitespace-nowrap font-medium select-none",
        radius::MD,
        TRANSITION_COLORS_CLASSES,
        FOCUS_RING_CLASSES,
        DISABLED_CONTROL_CLASSES,
        elevation,
        size_cls,
        variant_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for text input controls.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn input_classes(size: Size, invalid: bool, custom: Option<&str>) -> String {
    let size_cls = match size {
        Size::Xs | Size::Sm => "h-8 text-xs px-2.5 py-1",
        Size::Md => "h-9 text-sm px-3 py-1",
        Size::Lg | Size::Xl | Size::Icon => "h-10 text-sm px-4 py-2",
    };

    let state_cls = if invalid {
        "border-destructive focus-visible:ring-destructive"
    } else {
        "border-input focus-visible:ring-ring"
    };

    merge_classes(&[
        "flex w-full border bg-background text-foreground",
        "file:border-0 file:bg-transparent file:text-sm file:font-medium",
        "placeholder:text-muted-foreground",
        radius::MD,
        shadow::XS,
        TRANSITION_COLORS_CLASSES,
        INPUT_FOCUS_RING_CLASSES,
        DISABLED_INPUT_CLASSES,
        size_cls,
        state_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for multiline text areas.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn textarea_classes(size: Size, invalid: bool, custom: Option<&str>) -> String {
    let size_cls = match size {
        Size::Xs | Size::Sm => "text-xs px-2.5 py-1.5",
        Size::Md => "text-sm px-3 py-2",
        Size::Lg | Size::Xl | Size::Icon => "text-sm px-4 py-3",
    };

    let state_cls = if invalid {
        "border-destructive focus-visible:ring-destructive"
    } else {
        "border-input focus-visible:ring-ring"
    };

    merge_classes(&[
        "flex w-full border bg-background text-foreground",
        "placeholder:text-muted-foreground resize-y",
        radius::MD,
        shadow::XS,
        TRANSITION_COLORS_CLASSES,
        INPUT_FOCUS_RING_CLASSES,
        DISABLED_INPUT_CLASSES,
        size_cls,
        state_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for select drop-downs.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn select_classes(size: Size, invalid: bool, custom: Option<&str>) -> String {
    let size_cls = match size {
        Size::Xs | Size::Sm => "h-8 text-xs px-2.5",
        Size::Md => "h-9 text-sm px-3 py-1",
        Size::Lg | Size::Xl | Size::Icon => "h-10 text-sm px-4 py-2",
    };

    let state_cls = if invalid {
        "border-destructive focus:ring-destructive"
    } else {
        "border-input focus:ring-ring"
    };

    merge_classes(&[
        "flex w-full border bg-background text-foreground",
        radius::MD,
        shadow::XS,
        TRANSITION_COLORS_CLASSES,
        "focus:outline-none focus:ring-1",
        DISABLED_INPUT_CLASSES,
        size_cls,
        state_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for checkboxes.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn checkbox_classes(custom: Option<&str>) -> String {
    merge_classes(&[
        "h-4 w-4 border border-primary text-primary cursor-pointer",
        "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring",
        radius::SM,
        DISABLED_INPUT_CLASSES,
        custom.unwrap_or(""),
    ])
}

/// Generates CSS classes (track, thumb) for switch controls.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn switch_classes(checked: bool, size: SwitchSize, custom: Option<&str>) -> (String, String) {
    let (track_dim, thumb_dim, thumb_offset) = match size {
        SwitchSize::Sm => ("w-7 h-4", "h-3 w-3", "translate-x-3"),
        SwitchSize::Md => ("w-11 h-6", "h-5 w-5", "translate-x-5"),
        SwitchSize::Lg => ("w-14 h-7", "h-6 w-6", "translate-x-7"),
    };

    let track_bg = if checked { "bg-primary" } else { "bg-input" };
    let thumb_trans = if checked {
        thumb_offset
    } else {
        "translate-x-0"
    };

    let track = merge_classes(&[
        "peer inline-flex shrink-0 cursor-pointer items-center border-2 border-transparent",
        radius::FULL,
        shadow::XS,
        FOCUS_RING_CLASSES,
        TRANSITION_COLORS_CLASSES,
        DISABLED_CONTROL_CLASSES,
        track_bg,
        track_dim,
        custom.unwrap_or(""),
    ]);

    let thumb = merge_classes(&[
        "pointer-events-none block bg-background ring-0 transition-transform",
        radius::FULL,
        shadow::LG,
        thumb_dim,
        thumb_trans,
    ]);

    (track, thumb)
}

/// Generates a CSS class string for status badges and tags.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn badge_classes(variant: BadgeVariant, size: Size, custom: Option<&str>) -> String {
    let size_cls = match size {
        Size::Xs => "px-1.5 py-0 text-[9px] gap-0.5",
        Size::Sm => "px-1.5 py-0 text-[10px] gap-1",
        Size::Md => "px-2.5 py-0.5 text-xs gap-1.5",
        Size::Lg | Size::Xl | Size::Icon => "px-3 py-1 text-sm gap-2",
    };

    let variant_cls = match variant {
        BadgeVariant::Default => {
            "border-transparent bg-primary text-primary-foreground hover:bg-primary/80"
        }
        BadgeVariant::Secondary => {
            "border-transparent bg-secondary text-secondary-foreground hover:bg-secondary/80"
        }
        BadgeVariant::Destructive => {
            "border-transparent bg-destructive text-destructive-foreground hover:bg-destructive/80"
        }
        BadgeVariant::Outline => "text-foreground border-border",
        BadgeVariant::Success => {
            "border-transparent bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
        }
        BadgeVariant::Warning => {
            "border-transparent bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400"
        }
        BadgeVariant::Info => {
            "border-transparent bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400"
        }
    };

    let elevation = match variant {
        BadgeVariant::Default | BadgeVariant::Destructive => shadow::XS,
        _ => shadow::NONE,
    };

    merge_classes(&[
        "inline-flex items-center border font-semibold select-none",
        radius::FULL,
        TRANSITION_COLORS_CLASSES,
        elevation,
        size_cls,
        variant_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for alert banners.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn alert_classes(variant: AlertVariant, custom: Option<&str>) -> String {
    let variant_cls = match variant {
        AlertVariant::Default => "border-border bg-card text-card-foreground",
        AlertVariant::Info => {
            "border-blue-200 bg-blue-50 text-blue-800 dark:border-blue-800 dark:bg-blue-950 dark:text-blue-200"
        }
        AlertVariant::Warning => {
            "border-amber-300 bg-amber-50 text-amber-800 dark:border-amber-700 dark:bg-amber-950 dark:text-amber-200"
        }
        AlertVariant::Destructive => {
            "border-red-300 bg-red-50 text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200"
        }
        AlertVariant::Success => {
            "border-emerald-200 bg-emerald-50 text-emerald-800 dark:border-emerald-800 dark:bg-emerald-950 dark:text-emerald-200"
        }
    };

    merge_classes(&[
        "relative w-full border px-4 py-3 text-sm",
        radius::LG,
        variant_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for card panels.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn card_classes(variant: CardVariant, custom: Option<&str>) -> String {
    let variant_cls = match variant {
        CardVariant::Default => "border bg-card text-card-foreground",
        CardVariant::Bordered => "border-2 border-border bg-card text-card-foreground",
        CardVariant::Elevated => "border bg-card text-card-foreground",
        CardVariant::Ghost => "bg-transparent text-card-foreground",
    };

    let elevation = match variant {
        CardVariant::Default => shadow::SM,
        CardVariant::Elevated => shadow::MD,
        CardVariant::Bordered | CardVariant::Ghost => shadow::NONE,
    };

    merge_classes(&[
        "flex flex-col gap-6 py-6",
        radius::XL,
        elevation,
        variant_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for the card header slot.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn card_header_classes(custom: Option<&str>) -> String {
    merge_classes(&[
        "grid auto-rows-min grid-rows-[auto_auto] items-start gap-1.5 px-6",
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for the card title slot.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn card_title_classes(custom: Option<&str>) -> String {
    merge_classes(&["font-semibold leading-none", custom.unwrap_or("")])
}

/// Generates a CSS class string for the card description slot.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn card_description_classes(custom: Option<&str>) -> String {
    merge_classes(&["text-sm text-muted-foreground", custom.unwrap_or("")])
}

/// Generates a CSS class string for the card action slot.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn card_action_classes(custom: Option<&str>) -> String {
    merge_classes(&[
        "col-start-2 row-span-2 row-start-1 self-start justify-self-end",
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for the card content slot.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn card_content_classes(custom: Option<&str>) -> String {
    merge_classes(&["px-6", custom.unwrap_or("")])
}

/// Generates a CSS class string for the card footer slot.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn card_footer_classes(custom: Option<&str>) -> String {
    merge_classes(&["flex items-center px-6", custom.unwrap_or("")])
}

/// Generates (container, fallback) classes for user avatars.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn avatar_classes(size: AvatarSize, custom: Option<&str>) -> (String, String) {
    let (dim_cls, font_cls) = match size {
        AvatarSize::Xs => ("h-6 w-6", "text-[10px]"),
        AvatarSize::Sm => ("h-8 w-8", "text-xs"),
        AvatarSize::Md => ("h-10 w-10", "text-sm"),
        AvatarSize::Lg => ("h-12 w-12", "text-base"),
        AvatarSize::Xl => ("h-16 w-16", "text-lg"),
    };

    let container = merge_classes(&[
        "relative flex shrink-0 overflow-hidden select-none",
        radius::FULL,
        dim_cls,
        custom.unwrap_or(""),
    ]);

    let fallback = merge_classes(&[
        "flex h-full w-full items-center justify-center bg-muted font-medium text-muted-foreground",
        radius::FULL,
        font_cls,
    ]);

    (container, fallback)
}

/// Generates a CSS class string for skeleton loading placeholders.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn skeleton_classes(variant: SkeletonVariant, custom: Option<&str>) -> String {
    let shape_cls = match variant {
        SkeletonVariant::Text => "h-4 w-full",
        SkeletonVariant::Circular => "",
        SkeletonVariant::Rectangular => "",
    };

    let shape_radius = match variant {
        SkeletonVariant::Text => radius::SM,
        SkeletonVariant::Circular => radius::FULL,
        SkeletonVariant::Rectangular => radius::MD,
    };

    merge_classes(&[
        "animate-pulse bg-muted",
        shape_radius,
        shape_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for determinate progress bars.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn progress_classes(custom: Option<&str>) -> String {
    merge_classes(&[
        "relative h-2 w-full overflow-hidden rounded-full bg-primary/20",
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for spinner progress indicators.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn spinner_classes(size: Size, custom: Option<&str>) -> String {
    let size_cls = match size {
        Size::Xs => "h-3 w-3 border",
        Size::Sm => "h-4 w-4 border-2",
        Size::Md => "h-6 w-6 border-2",
        Size::Lg => "h-8 w-8 border-[3px]",
        Size::Xl => "h-10 w-10 border-4",
        Size::Icon => "h-5 w-5 border-2",
    };

    merge_classes(&[
        "inline-block border-primary border-t-transparent animate-spin",
        radius::FULL,
        size_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for field labels.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn label_classes(disabled: bool, custom: Option<&str>) -> String {
    let disabled_cls = if disabled {
        "cursor-not-allowed opacity-50"
    } else {
        "peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
    };

    merge_classes(&[
        "text-sm font-medium leading-none",
        disabled_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for horizontal/vertical separators.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn separator_classes(orientation: Orientation, custom: Option<&str>) -> String {
    let dim_cls = match orientation {
        Orientation::Horizontal => "w-full h-px",
        Orientation::Vertical => "h-full w-px",
    };

    merge_classes(&["shrink-0 bg-border", dim_cls, custom.unwrap_or("")])
}

/// Generates a CSS class string for modal dialog backdrop overlays.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn dialog_backdrop_classes(open: bool) -> String {
    let state_cls = if open {
        "opacity-100 pointer-events-auto"
    } else {
        "opacity-0 pointer-events-none"
    };

    merge_classes(&[
        "fixed inset-0 z-50 bg-black/80 backdrop-blur-xs transition-opacity duration-200",
        state_cls,
    ])
}

/// Generates a CSS class string for modal dialog content containers.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn dialog_content_classes(open: bool, custom: Option<&str>) -> String {
    let state_cls = if open {
        "opacity-100 scale-100 pointer-events-auto"
    } else {
        "opacity-0 scale-95 pointer-events-none"
    };

    merge_classes(&[
        "fixed left-1/2 top-1/2 z-50 grid w-full max-w-lg -translate-x-1/2 -translate-y-1/2",
        "gap-4 border bg-background p-6 duration-200",
        radius::LG,
        shadow::LG,
        state_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for the dialog's built-in close control.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn dialog_close_classes(custom: Option<&str>) -> String {
    merge_classes(&[
        "ring-offset-background focus:ring-ring data-[state=open]:bg-accent data-[state=open]:text-muted-foreground absolute right-4 top-4 rounded-xs opacity-70 transition-opacity hover:opacity-100 focus:ring-2 focus:ring-offset-2 focus:outline-hidden disabled:pointer-events-none [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for the dialog header slot.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn dialog_header_classes(custom: Option<&str>) -> String {
    merge_classes(&[
        "flex flex-col space-y-1.5 text-center sm:text-left",
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for the dialog title slot.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn dialog_title_classes(custom: Option<&str>) -> String {
    merge_classes(&[
        "text-lg font-semibold leading-none tracking-tight",
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for the dialog description slot.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn dialog_description_classes(custom: Option<&str>) -> String {
    merge_classes(&["text-sm text-muted-foreground", custom.unwrap_or("")])
}

/// Generates a CSS class string for the dialog footer slot.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn dialog_footer_classes(custom: Option<&str>) -> String {
    merge_classes(&[
        "flex flex-col-reverse sm:flex-row sm:justify-end sm:space-x-2",
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for tabs list containers.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn tabs_list_classes(orientation: Orientation, custom: Option<&str>) -> String {
    let orient_cls = match orientation {
        Orientation::Horizontal => "inline-flex h-9 items-center justify-center p-1",
        Orientation::Vertical => "inline-flex flex-col items-stretch p-1",
    };

    merge_classes(&[
        "bg-muted text-muted-foreground",
        radius::LG,
        orient_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for tab button triggers.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn tabs_trigger_classes(active: bool, disabled: bool, custom: Option<&str>) -> String {
    let state_cls = if active {
        merge_classes(&["bg-background text-foreground font-semibold", shadow::XS])
    } else {
        "hover:bg-background/50 hover:text-foreground font-normal".to_string()
    };

    let disabled_cls = if disabled {
        "pointer-events-none opacity-50"
    } else {
        ""
    };

    merge_classes(&[
        "inline-flex items-center justify-center whitespace-nowrap px-3 py-1 text-sm transition-all",
        radius::MD,
        FOCUS_RING_CLASSES,
        state_cls.as_str(),
        disabled_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for tab content panels.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn tabs_content_classes(active: bool, custom: Option<&str>) -> String {
    let display_cls = if active { "block" } else { "hidden" };

    merge_classes(&[
        "mt-2",
        FOCUS_RING_CLASSES,
        display_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates a CSS class string for the Table of Contents container navigation.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn toc_nav_classes(sticky: bool, custom: Option<&str>) -> String {
    let sticky_cls = if sticky { "sticky top-24" } else { "" };
    merge_classes(&[
        sticky_cls,
        "rounded-2xl border border-border bg-card p-5 shadow-sm space-y-3",
        custom.unwrap_or(""),
    ])
}

/// Returns CSS classes for the Table of Contents header banner.
#[must_use]
pub fn toc_header_classes() -> &'static str {
    "flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground border-b border-border pb-3"
}

/// Returns CSS classes for the Table of Contents scrollable list container.
#[must_use]
pub fn toc_list_classes() -> &'static str {
    "space-y-1.5 text-sm max-h-[calc(100vh-12rem)] overflow-y-auto pr-1"
}

/// Generates a CSS class string for a Table of Contents link item.
#[must_use = "the resolved class list has no effect unless it is applied to an element"]
pub fn toc_item_classes(level: u8, active: bool) -> String {
    let padding_cls = if level >= 4 {
        "pl-6"
    } else if level == 3 {
        "pl-3"
    } else {
        ""
    };

    let state_cls = if active {
        "bg-primary/10 font-semibold text-primary border-l-2 border-primary"
    } else {
        "text-muted-foreground hover:bg-muted hover:text-foreground"
    };

    merge_classes(&[
        "block py-1 text-xs transition-colors rounded-md px-2",
        padding_cls,
        state_cls,
    ])
}
