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

use crate::tokens::{DISABLED_CONTROL_CLASSES, FOCUS_RING_CLASSES, TRANSITION_COLORS_CLASSES};
use crate::types::{
    AlertVariant, AvatarSize, BadgeVariant, ButtonVariant, CardVariant, Orientation,
    SkeletonVariant, Size, SwitchSize,
};

/// Helper to merge multiple CSS class fragments into a clean string.
fn merge_classes(parts: &[&str]) -> String {
    parts
        .iter()
        .filter(|s| !s.trim().is_empty())
        .copied()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Generates CSS class string for a button.
pub fn button_classes(
    variant: ButtonVariant,
    size: Size,
    _disabled: bool,
    _loading: bool,
    custom: Option<&str>,
) -> String {
    let size_cls = match size {
        Size::Xs => "h-7 px-2.5 text-xs gap-1",
        Size::Sm => "h-8 px-3 text-xs gap-1.5",
        Size::Md => "h-9 px-4 py-2 text-sm gap-2",
        Size::Lg => "h-10 px-8 text-sm gap-2",
        Size::Xl => "h-12 px-10 text-base gap-2.5",
        Size::Icon => "h-9 w-9 p-0",
    };

    let variant_cls = match variant {
        ButtonVariant::Default => "bg-primary text-primary-foreground shadow hover:bg-primary/90",
        ButtonVariant::Destructive => {
            "bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90"
        }
        ButtonVariant::Outline => {
            "border border-input bg-background shadow-sm hover:bg-accent hover:text-accent-foreground"
        }
        ButtonVariant::Secondary => {
            "bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80"
        }
        ButtonVariant::Ghost => "hover:bg-accent hover:text-accent-foreground",
        ButtonVariant::Link => "text-primary underline-offset-4 hover:underline",
    };

    let base = "inline-flex items-center justify-center whitespace-nowrap rounded-md font-medium select-none";

    merge_classes(&[
        base,
        TRANSITION_COLORS_CLASSES,
        FOCUS_RING_CLASSES,
        DISABLED_CONTROL_CLASSES,
        size_cls,
        variant_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates CSS class string for text input controls.
pub fn input_classes(
    size: Size,
    invalid: bool,
    _disabled: bool,
    custom: Option<&str>,
) -> String {
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

    let base = "flex w-full rounded-md border bg-background text-foreground shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 disabled:cursor-not-allowed disabled:opacity-50";

    merge_classes(&[base, size_cls, state_cls, custom.unwrap_or("")])
}

/// Generates CSS class string for multiline text areas.
pub fn textarea_classes(
    size: Size,
    invalid: bool,
    _disabled: bool,
    custom: Option<&str>,
) -> String {
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

    let base = "flex w-full rounded-md border bg-background text-foreground shadow-sm placeholder:text-muted-foreground resize-y focus-visible:outline-none focus-visible:ring-1 disabled:cursor-not-allowed disabled:opacity-50";

    merge_classes(&[base, size_cls, state_cls, custom.unwrap_or("")])
}

/// Generates CSS class string for select drop-downs.
pub fn select_classes(
    size: Size,
    invalid: bool,
    _disabled: bool,
    custom: Option<&str>,
) -> String {
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

    let base = "flex w-full rounded-md border bg-background text-foreground shadow-sm focus:outline-none focus:ring-1 disabled:cursor-not-allowed disabled:opacity-50";

    merge_classes(&[base, size_cls, state_cls, custom.unwrap_or("")])
}

/// Generates CSS class string for checkboxes.
pub fn checkbox_classes(_disabled: bool, custom: Option<&str>) -> String {
    let base = "h-4 w-4 rounded border border-primary text-primary shadow focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 cursor-pointer";
    merge_classes(&[base, custom.unwrap_or("")])
}

/// Generates CSS classes (track, thumb) for switch controls.
pub fn switch_classes(
    checked: bool,
    size: SwitchSize,
    _disabled: bool,
    custom: Option<&str>,
) -> (String, String) {
    let (track_dim, thumb_dim, thumb_offset) = match size {
        SwitchSize::Sm => ("w-7 h-4", "h-3 w-3", "translate-x-3"),
        SwitchSize::Md => ("w-11 h-6", "h-5 w-5", "translate-x-5"),
        SwitchSize::Lg => ("w-14 h-7", "h-6 w-6", "translate-x-7"),
    };

    let track_bg = if checked { "bg-primary" } else { "bg-input" };
    let thumb_trans = if checked { thumb_offset } else { "translate-x-0" };

    let track_base = "peer inline-flex shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent shadow-sm transition-colors disabled:cursor-not-allowed disabled:opacity-50";
    let thumb_base = "pointer-events-none block rounded-full bg-background shadow-lg ring-0 transition-transform";

    let track = merge_classes(&[
        track_base,
        FOCUS_RING_CLASSES,
        track_bg,
        track_dim,
        custom.unwrap_or(""),
    ]);

    let thumb = merge_classes(&[thumb_base, thumb_dim, thumb_trans]);

    (track, thumb)
}

/// Generates CSS class string for status badges and tags.
pub fn badge_classes(variant: BadgeVariant, size: Size, custom: Option<&str>) -> String {
    let size_cls = match size {
        Size::Xs => "px-1.5 py-0 text-[9px] gap-0.5",
        Size::Sm => "px-1.5 py-0 text-[10px] gap-1",
        Size::Md => "px-2.5 py-0.5 text-xs gap-1.5",
        Size::Lg | Size::Xl | Size::Icon => "px-3 py-1 text-sm gap-2",
    };

    let variant_cls = match variant {
        BadgeVariant::Default => {
            "border-transparent bg-primary text-primary-foreground shadow hover:bg-primary/80"
        }
        BadgeVariant::Secondary => {
            "border-transparent bg-secondary text-secondary-foreground hover:bg-secondary/80"
        }
        BadgeVariant::Destructive => {
            "border-transparent bg-destructive text-destructive-foreground shadow hover:bg-destructive/80"
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

    let base = "inline-flex items-center rounded-full border font-semibold select-none transition-colors";

    merge_classes(&[base, size_cls, variant_cls, custom.unwrap_or("")])
}

/// Generates CSS class string for alert banners.
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

    let base = "relative w-full rounded-lg border px-4 py-3 text-sm";
    merge_classes(&[base, variant_cls, custom.unwrap_or("")])
}

/// Generates CSS class string for card panels.
pub fn card_classes(variant: CardVariant, custom: Option<&str>) -> String {
    let variant_cls = match variant {
        CardVariant::Default => "border bg-card text-card-foreground shadow-sm",
        CardVariant::Bordered => "border-2 border-border bg-card text-card-foreground",
        CardVariant::Elevated => "border bg-card text-card-foreground shadow-md",
        CardVariant::Ghost => "bg-transparent text-card-foreground",
    };

    let base = "flex flex-col gap-6 rounded-xl py-6";
    merge_classes(&[base, variant_cls, custom.unwrap_or("")])
}

pub fn card_header_classes(custom: Option<&str>) -> String {
    merge_classes(&[
        "grid auto-rows-min grid-rows-[auto_auto] items-start gap-1.5 px-6",
        custom.unwrap_or(""),
    ])
}

pub fn card_title_classes(custom: Option<&str>) -> String {
    merge_classes(&["font-semibold leading-none", custom.unwrap_or("")])
}

pub fn card_description_classes(custom: Option<&str>) -> String {
    merge_classes(&["text-sm text-muted-foreground", custom.unwrap_or("")])
}

pub fn card_action_classes(custom: Option<&str>) -> String {
    merge_classes(&[
        "col-start-2 row-span-2 row-start-1 self-start justify-self-end",
        custom.unwrap_or(""),
    ])
}

pub fn card_content_classes(custom: Option<&str>) -> String {
    merge_classes(&["px-6", custom.unwrap_or("")])
}

pub fn card_footer_classes(custom: Option<&str>) -> String {
    merge_classes(&["flex items-center px-6", custom.unwrap_or("")])
}

/// Generates (container, fallback) classes for user avatars.
pub fn avatar_classes(size: AvatarSize, custom: Option<&str>) -> (String, String) {
    let (dim_cls, font_cls) = match size {
        AvatarSize::Xs => ("h-6 w-6", "text-[10px]"),
        AvatarSize::Sm => ("h-8 w-8", "text-xs"),
        AvatarSize::Md => ("h-10 w-10", "text-sm"),
        AvatarSize::Lg => ("h-12 w-12", "text-base"),
        AvatarSize::Xl => ("h-16 w-16", "text-lg"),
    };

    let container = merge_classes(&[
        "relative flex shrink-0 overflow-hidden rounded-full select-none",
        dim_cls,
        custom.unwrap_or(""),
    ]);

    let fallback = merge_classes(&[
        "flex h-full w-full items-center justify-center rounded-full bg-muted font-medium text-muted-foreground",
        font_cls,
    ]);

    (container, fallback)
}

/// Generates CSS class string for skeleton loading placeholders.
pub fn skeleton_classes(variant: SkeletonVariant, custom: Option<&str>) -> String {
    let shape_cls = match variant {
        SkeletonVariant::Text => "h-4 w-full rounded",
        SkeletonVariant::Circular => "rounded-full",
        SkeletonVariant::Rectangular => "rounded-md",
    };

    let base = "animate-pulse bg-muted";
    merge_classes(&[base, shape_cls, custom.unwrap_or("")])
}

/// Generates CSS class string for spinner progress indicators.
pub fn spinner_classes(size: Size, custom: Option<&str>) -> String {
    let size_cls = match size {
        Size::Xs => "h-3 w-3 border",
        Size::Sm => "h-4 w-4 border-2",
        Size::Md => "h-6 w-6 border-2",
        Size::Lg => "h-8 w-8 border-[3px]",
        Size::Xl => "h-10 w-10 border-4",
        Size::Icon => "h-5 w-5 border-2",
    };

    let base = "inline-block rounded-full border-primary border-t-transparent animate-spin";
    merge_classes(&[base, size_cls, custom.unwrap_or("")])
}

/// Generates CSS class string for field labels.
pub fn label_classes(_required: bool, disabled: bool, custom: Option<&str>) -> String {
    let disabled_cls = if disabled {
        "cursor-not-allowed opacity-50"
    } else {
        "peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
    };

    let base = "text-sm font-medium leading-none";
    merge_classes(&[base, disabled_cls, custom.unwrap_or("")])
}

/// Generates CSS class string for horizontal/vertical separators.
pub fn separator_classes(orientation: Orientation, custom: Option<&str>) -> String {
    let dim_cls = match orientation {
        Orientation::Horizontal => "w-full h-px",
        Orientation::Vertical => "h-full w-px",
    };

    let base = "shrink-0 bg-border";
    merge_classes(&[base, dim_cls, custom.unwrap_or("")])
}

/// Generates CSS class string for modal dialog backdrop overlays.
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

/// Generates CSS class string for modal dialog content containers.
pub fn dialog_content_classes(open: bool, custom: Option<&str>) -> String {
    let state_cls = if open {
        "opacity-100 scale-100 pointer-events-auto"
    } else {
        "opacity-0 scale-95 pointer-events-none"
    };

    merge_classes(&[
        "fixed left-1/2 top-1/2 z-50 grid w-full max-w-lg -translate-x-1/2 -translate-y-1/2 gap-4 border bg-background p-6 shadow-lg duration-200 rounded-lg",
        state_cls,
        custom.unwrap_or(""),
    ])
}

pub fn dialog_header_classes(custom: Option<&str>) -> String {
    merge_classes(&[
        "flex flex-col space-y-1.5 text-center sm:text-left",
        custom.unwrap_or(""),
    ])
}

pub fn dialog_title_classes(custom: Option<&str>) -> String {
    merge_classes(&[
        "text-lg font-semibold leading-none tracking-tight",
        custom.unwrap_or(""),
    ])
}

pub fn dialog_description_classes(custom: Option<&str>) -> String {
    merge_classes(&["text-sm text-muted-foreground", custom.unwrap_or("")])
}

pub fn dialog_footer_classes(custom: Option<&str>) -> String {
    merge_classes(&[
        "flex flex-col-reverse sm:flex-row sm:justify-end sm:space-x-2",
        custom.unwrap_or(""),
    ])
}

/// Generates CSS class string for tabs list container.
pub fn tabs_list_classes(orientation: Orientation, custom: Option<&str>) -> String {
    let orient_cls = match orientation {
        Orientation::Horizontal => "inline-flex h-9 items-center justify-center rounded-lg bg-muted p-1 text-muted-foreground",
        Orientation::Vertical => "inline-flex flex-col items-stretch rounded-lg bg-muted p-1 text-muted-foreground",
    };

    merge_classes(&[orient_cls, custom.unwrap_or("")])
}

/// Generates CSS class string for tab button triggers.
pub fn tabs_trigger_classes(active: bool, disabled: bool, custom: Option<&str>) -> String {
    let state_cls = if active {
        "bg-background text-foreground shadow-xs font-semibold"
    } else {
        "hover:bg-background/50 hover:text-foreground font-normal"
    };

    let disabled_cls = if disabled {
        "pointer-events-none opacity-50"
    } else {
        ""
    };

    merge_classes(&[
        "inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
        state_cls,
        disabled_cls,
        custom.unwrap_or(""),
    ])
}

/// Generates CSS class string for tab content panel.
pub fn tabs_content_classes(active: bool, custom: Option<&str>) -> String {
    let display_cls = if active { "block" } else { "hidden" };
    merge_classes(&[
        "mt-2 ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
        display_cls,
        custom.unwrap_or(""),
    ])
}
