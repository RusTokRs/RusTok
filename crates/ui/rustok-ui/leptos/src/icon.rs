/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Inline icon set.
//!
//! AGENTS.md requires icons to come from the design system, and the audit found the opposite:
//! thirty-nine hand-written `<svg>` blocks across the admin and the module crates, each with its own
//! stroke width, size attribute and `aria-hidden` decision. One of them was an icon *and* a button
//! label; another announced itself twice to a screen reader.
//!
//! The icons here are Lucide shapes, inlined as path data. Where the admin already drew the glyph
//! (`Shield`, `Package`), that exact geometry is kept, so converting a call site is pixel-identical
//! rather than "close enough". Lucide geometry is MIT-licensed, and inlining it beats a dependency:
//! an icon-font or SVG-sprite build step in a Leptos/WASM frontend, for two dozen paths, is a build
//! pipeline nobody wants to own.
//!
//! Every icon is a `24×24` view box drawn with `currentColor`, so colour comes from the surrounding
//! text class and the icon inherits `disabled:`/`text-muted-foreground` for free.
//!
//! Accessibility is decided once, here: an icon is decorative (`aria-hidden="true"`) unless the
//! caller passes `aria_label`, in which case it becomes an image with a name. There is no third
//! state, because "sometimes announced" is what produced the double announcement in the first place.

use leptos::prelude::*;

use rustok_ui::merge_classes;

/// The icons the design system ships.
///
/// The set is deliberately small: it is the vocabulary the admin already draws by hand, not a
/// general-purpose icon library. Adding a shape means adding path data here so every host gets the
/// same geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconName {
    /// Checkmark.
    Check,
    /// Chevron pointing down.
    ChevronDown,
    /// Chevron pointing left.
    ChevronLeft,
    /// Chevron pointing right.
    ChevronRight,
    /// Chevron pointing up.
    ChevronUp,
    /// Open eye (visible).
    Eye,
    /// Crossed-out eye (hidden).
    EyeOff,
    /// Key.
    Key,
    /// Padlock.
    Lock,
    /// Crescent moon (dark theme).
    Moon,
    /// Sidebar / panel on the left.
    PanelLeft,
    /// Plus sign.
    Plus,
    /// Magnifying glass.
    Search,
    /// Shield with a checkmark.
    ShieldCheck,
    /// Sun (light theme).
    Sun,
    /// Command-line prompt.
    Terminal,
    /// Single person.
    User,
    /// Group of people.
    Users,
    /// Package / box, drawn as the admin already draws it.
    Package,
    /// Plain shield.
    Shield,
    /// Close cross.
    X,
}

impl IconName {
    /// SVG path data, in draw order.
    ///
    /// Circles and rectangles are expressed as paths: a `d` attribute is the one shape every icon
    /// uses, so rendering stays a single loop and a future icon cannot accidentally need a new
    /// element type.
    pub fn paths(self) -> &'static [&'static str] {
        match self {
            Self::Check => &["M20 6 9 17l-5-5"],
            Self::ChevronDown => &["m6 9 6 6 6-6"],
            Self::ChevronLeft => &["m15 18-6-6 6-6"],
            Self::ChevronRight => &["m9 18 6-6-6-6"],
            Self::ChevronUp => &["m18 15-6-6-6 6"],
            Self::Eye => &[
                "M2.06 12.35a1 1 0 0 1 0-.7 10.75 10.75 0 0 1 19.88 0 1 1 0 0 1 0 .7 10.75 10.75 0 0 1-19.88 0",
                "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6",
            ],
            Self::EyeOff => &[
                "M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94m3.84-1.82A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24",
                "M1 1l22 22",
            ],
            Self::Key => &[
                "m15.5 7.5 2.3 2.3a1 1 0 0 0 1.4 0l2.1-2.1a1 1 0 0 0 0-1.4L19 4",
                "m21 2-9.6 9.6",
                "M7.5 21a5.5 5.5 0 1 0 0-11 5.5 5.5 0 0 0 0 11",
            ],
            Self::Lock => &[
                "M5 11h14a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2Z",
                "M7 11V7a5 5 0 0 1 10 0v4",
            ],
            Self::Moon => &["M12 3a6.9 6.9 0 0 0 9 9 9 9 0 1 1-9-9Z"],
            Self::PanelLeft => &[
                "M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z",
                "M9 3v18",
            ],
            Self::Plus => &["M5 12h14", "M12 5v14"],
            Self::Search => &["M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16", "m21 21-4.3-4.3"],
            Self::ShieldCheck => &[
                "M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z",
                "m9 12 2 2 4-4",
            ],
            Self::Sun => &[
                "M12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8",
                "M12 2v2",
                "M12 20v2",
                "m4.93 4.93 1.41 1.41",
                "m17.66 17.66 1.41 1.41",
                "M2 12h2",
                "M20 12h2",
                "m6.34 17.66-1.41 1.41",
                "m19.07 4.93-1.41 1.41",
            ],
            Self::Terminal => &["m4 17 6-6-6-6", "M12 19h8"],
            Self::User => &[
                "M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2",
                "M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8",
            ],
            Self::Users => &[
                "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2",
                "M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8",
                "M22 21v-2a4 4 0 0 0-3-3.87",
                "M16 3.13a4 4 0 0 1 0 7.75",
            ],
            Self::Package => &["M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"],
            Self::Shield => &["M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"],
            Self::X => &["M18 6 6 18", "m6 6 12 12"],
        }
    }
}

/// Inline icon rendered from [`IconName`].
///
/// Decorative by default (`aria-hidden="true"`). Pass `aria_label` only when the icon is the *only*
/// thing carrying a meaning — a labelled button does not need one, because the label already names
/// the control.
#[component]
pub fn Icon(
    name: IconName,
    /// Edge length in pixels. Icons are square, so there is no height prop.
    #[prop(default = 16)]
    size: u32,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] aria_label: String,
) -> impl IntoView {
    let label = (!aria_label.trim().is_empty()).then_some(aria_label);
    let decorative = label.is_none();
    let custom = (!class.trim().is_empty()).then_some(class.as_str());
    let full_class = merge_classes(&["shrink-0", custom.unwrap_or_default()]);

    view! {
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width=size
            height=size
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            data-slot="icon"
            class=full_class
            role=label.as_ref().map(|_| "img")
            aria-hidden=decorative.then_some("true")
            aria-label=label
        >
            {name.paths().iter().map(|path| view! { <path d=*path /> }).collect_view()}
        </svg>
    }
}
