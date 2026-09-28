/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Design tokens and CSS custom property semantics for RusToK UI.

/// Focus ring utility classes adhering to accessibility guidelines (WCAG 2.1 AA).
pub const FOCUS_RING_CLASSES: &str =
    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background";

/// Subtle focus ring for compact controls like inputs and selects.
pub const INPUT_FOCUS_RING_CLASSES: &str =
    "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring";

/// Transition classes for interactive elements.
pub const TRANSITION_COLORS_CLASSES: &str = "transition-colors duration-150 ease-in-out";

/// Disabled control utility classes.
pub const DISABLED_CONTROL_CLASSES: &str = "disabled:pointer-events-none disabled:opacity-50";

/// Standard border radius tokens.
pub mod radius {
    pub const FULL: &str = "rounded-full";
    pub const XL: &str = "rounded-xl";
    pub const LG: &str = "rounded-lg";
    pub const MD: &str = "rounded-md";
    pub const SM: &str = "rounded-sm";
}

/// Standard elevation/shadow tokens.
pub mod shadow {
    pub const NONE: &str = "shadow-none";
    pub const XS: &str = "shadow-xs";
    pub const SM: &str = "shadow-sm";
    pub const MD: &str = "shadow-md";
    pub const LG: &str = "shadow-lg";
}
