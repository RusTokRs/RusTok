/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # RusToK UI Core Design System
//!
//! Framework-agnostic design system primitives, variants, contracts, and styling resolvers.
//!
//! The crate is wrapped by the framework adapters `rustok-ui-leptos` (Leptos 0.8)
//! and `rustok-ui-dioxus` (Dioxus 0.7). Hosts that only need deterministic class
//! strings — for example a server-side renderer that emits HTML itself — can
//! depend on this crate directly.
//!
//! ```
//! use rustok_ui::{ButtonVariant, Size, button_classes};
//!
//! let classes = button_classes(ButtonVariant::Default, Size::Md, None);
//! assert!(classes.contains("bg-primary"));
//! ```
//!
//! Component *state* (disabled, loading, invalid, checked) is expressed by the
//! adapters through native HTML attributes (`disabled`, `aria-invalid`, …) and
//! the matching `disabled:`/`aria-` Tailwind variants, which is why the class
//! resolvers only take the static variant/size configuration plus optional
//! caller classes.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod classes;
pub mod contracts;
pub mod tokens;
pub mod types;

#[cfg(test)]
mod tests;

pub use classes::{
    alert_classes, avatar_classes, badge_classes, button_classes, card_action_classes,
    card_classes, card_content_classes, card_description_classes, card_footer_classes,
    card_header_classes, card_title_classes, checkbox_classes, dialog_backdrop_classes,
    dialog_close_classes, dialog_content_classes, dialog_description_classes,
    dialog_footer_classes, dialog_header_classes, dialog_title_classes, input_classes,
    label_classes, merge_classes, progress_classes, select_classes, separator_classes,
    skeleton_classes, spinner_classes, switch_classes, tabs_content_classes, tabs_list_classes,
    tabs_trigger_classes, textarea_classes, toc_header_classes, toc_item_classes,
    toc_list_classes, toc_nav_classes,
};
pub use contracts::{
    DialogState, SelectOption, TabItem, TabsState, TocItem, extract_headings_from_html,
    extract_initials, normalize_progress_max, normalize_progress_value,
    normalize_progress_value_for_max, progress_value_percentage, slugify_heading,
};
pub use tokens::{
    DISABLED_CONTROL_CLASSES, DISABLED_INPUT_CLASSES, FOCUS_RING_CLASSES, INPUT_FOCUS_RING_CLASSES,
    TRANSITION_COLORS_CLASSES, radius, shadow,
};
pub use types::{
    AlertVariant, AvatarSize, BadgeVariant, ButtonVariant, CardVariant, InputType, Orientation,
    Size, SkeletonVariant, SwitchSize,
};
