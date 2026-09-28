/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use serde::{Deserialize, Serialize};

/// Visual variant for buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ButtonVariant {
    #[default]
    Default,
    Destructive,
    Outline,
    Secondary,
    Ghost,
    Link,
}

/// Standard size scale for UI components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Size {
    Xs,
    Sm,
    #[default]
    Md,
    Lg,
    Xl,
    Icon,
}

/// Visual variant for alert banners and feedback containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AlertVariant {
    #[default]
    Default,
    Info,
    Warning,
    Destructive,
    Success,
}

/// Visual variant for badge tags and status chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BadgeVariant {
    #[default]
    Default,
    Secondary,
    Destructive,
    Outline,
    Success,
    Warning,
    Info,
}

/// Visual container style for card panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CardVariant {
    #[default]
    Default,
    Bordered,
    Elevated,
    Ghost,
}

/// Size scale for user avatars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AvatarSize {
    Xs,
    Sm,
    #[default]
    Md,
    Lg,
    Xl,
}

/// Geometric shape variant for skeleton loading placeholders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SkeletonVariant {
    #[default]
    Text,
    Circular,
    Rectangular,
}

/// HTML input type specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum InputType {
    #[default]
    Text,
    Password,
    Email,
    Number,
    Search,
    Tel,
    Url,
    Date,
}

impl InputType {
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
    #[default]
    Horizontal,
    Vertical,
}

impl Orientation {
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
    Sm,
    #[default]
    Md,
    Lg,
}
