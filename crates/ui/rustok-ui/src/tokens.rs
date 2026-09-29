/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Design tokens shared by every class resolver in [`crate::classes`].
//!
//! Tokens are Tailwind CSS v4 utility fragments. Keeping them in one module
//! guarantees that a focus ring, disabled state or elevation change lands in
//! every component at once instead of drifting per component.

/// Focus ring for primary interactive controls (buttons, switches, tab triggers).
pub const FOCUS_RING_CLASSES: &str = "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background";

/// Focus ring for text-like controls (inputs, textareas, selects).
///
/// The ring colour is intentionally not part of this token: callers append
/// `focus-visible:ring-ring` for valid controls or
/// `focus-visible:ring-destructive` for invalid ones.
pub const INPUT_FOCUS_RING_CLASSES: &str = "focus-visible:outline-none focus-visible:ring-1";

/// Transition applied to interactive elements.
pub const TRANSITION_COLORS_CLASSES: &str = "transition-colors duration-150 ease-in-out";

/// Disabled state for button-like controls that must not receive pointer events.
pub const DISABLED_CONTROL_CLASSES: &str = "disabled:pointer-events-none disabled:opacity-50";

/// Disabled state for form controls that keep native cursor semantics.
pub const DISABLED_INPUT_CLASSES: &str = "disabled:cursor-not-allowed disabled:opacity-50";

/// Border radius scale.
///
/// The scale mirrors the Tailwind CSS v4 `rounded-*` utilities used by the
/// design system: `SM` is the smallest step (the historical default radius)
/// and `FULL` is a pill.
pub mod radius {
    /// Pill-shaped radius.
    pub const FULL: &str = "rounded-full";
    /// Extra large radius.
    pub const XL: &str = "rounded-xl";
    /// Large radius.
    pub const LG: &str = "rounded-lg";
    /// Medium radius.
    pub const MD: &str = "rounded-md";
    /// Small (default) radius.
    pub const SM: &str = "rounded-sm";
}

/// Elevation/shadow scale.
///
/// The scale mirrors the Tailwind CSS v4 `shadow-*` utilities: `XS` is the
/// control elevation used by buttons and inputs, `MD`/`LG` are used by cards
/// and overlays.
pub mod shadow {
    /// No elevation.
    pub const NONE: &str = "shadow-none";
    /// Control elevation (buttons, inputs, badges).
    pub const XS: &str = "shadow-xs";
    /// Card elevation.
    pub const SM: &str = "shadow-sm";
    /// Raised panel elevation.
    pub const MD: &str = "shadow-md";
    /// Overlay elevation.
    pub const LG: &str = "shadow-lg";
}
