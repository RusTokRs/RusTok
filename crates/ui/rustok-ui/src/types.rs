/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Framework-independent component variants, sizes, and states.
//!
//! Every enum in this module describes a *visual* contract, never a framework
//! construct, so the same value can be handed to the Leptos adapter, the
//! Dioxus adapter, or serialized into a host configuration.

use serde::{Deserialize, Serialize};

/// Visual variant for buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ButtonVariant {
    /// Solid primary action.
    #[default]
    Default,
    /// Destructive action (delete, revoke).
    Destructive,
    /// Outlined secondary action.
    Outline,
    /// Filled low-emphasis action.
    Secondary,
    /// Borderless action for toolbars.
    Ghost,
    /// Inline text action.
    Link,
}

/// Standard size scale for UI components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Size {
    /// Extra small.
    Xs,
    /// Small.
    Sm,
    /// Medium (default).
    #[default]
    Md,
    /// Large.
    Lg,
    /// Extra large.
    Xl,
    /// Square icon-only control.
    Icon,
}

/// Visual variant for alert banners and feedback containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AlertVariant {
    /// Neutral container.
    #[default]
    Default,
    /// Informational message.
    Info,
    /// Warning that needs attention but is not fatal.
    Warning,
    /// Error or blocking failure.
    Destructive,
    /// Confirmation of a successful operation.
    Success,
}

/// Visual variant for badge tags and status chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BadgeVariant {
    /// Solid primary chip.
    #[default]
    Default,
    /// Filled low-emphasis chip.
    Secondary,
    /// Error/failure chip.
    Destructive,
    /// Outlined neutral chip.
    Outline,
    /// Operational success chip.
    Success,
    /// Degraded/warning chip.
    Warning,
    /// Informational chip.
    Info,
}

/// Visual container style for card panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CardVariant {
    /// Bordered panel with subtle elevation.
    #[default]
    Default,
    /// Panel with a heavier border and no elevation.
    Bordered,
    /// Raised panel for emphasis.
    Elevated,
    /// Transparent panel without chrome.
    Ghost,
}

/// Size scale for user avatars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AvatarSize {
    /// Extra small avatar.
    Xs,
    /// Small avatar.
    Sm,
    /// Medium (default) avatar.
    #[default]
    Md,
    /// Large avatar.
    Lg,
    /// Extra large avatar.
    Xl,
}

/// Geometric shape variant for skeleton loading placeholders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SkeletonVariant {
    /// Single line of text.
    #[default]
    Text,
    /// Circle, sized by the caller.
    Circular,
    /// Rectangle with the caller-provided dimensions.
    Rectangular,
}

/// HTML input type specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum InputType {
    /// `type="text"`.
    #[default]
    Text,
    /// `type="password"`.
    Password,
    /// `type="email"`.
    Email,
    /// `type="number"`.
    Number,
    /// `type="search"`.
    Search,
    /// `type="tel"`.
    Tel,
    /// `type="url"`.
    Url,
    /// `type="date"`.
    Date,
}

impl InputType {
    /// Returns the HTML attribute value for this input type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Password => "password",
            Self::Email => "email",
            Self::Number => "number",
            Self::Search => "search",
            Self::Tel => "tel",
            Self::Url => "url",
            Self::Date => "date",
        }
    }
}

/// Orientation axis for dividers, tabs, and layout stacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Orientation {
    /// Horizontal axis (default).
    #[default]
    Horizontal,
    /// Vertical axis.
    Vertical,
}

impl Orientation {
    /// Returns the ARIA/HTML attribute value for this orientation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// Size variant for switch toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SwitchSize {
    /// Small switch.
    Sm,
    /// Medium (default) switch.
    #[default]
    Md,
    /// Large switch.
    Lg,
}
