/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use dioxus::prelude::*;
use rustok_ui::{Size, spinner_classes};

/// Animated progress indicator.
///
/// The accessible name defaults to `Loading`; pass `aria_label` with a
/// localized string when the host renders more than one locale.
#[component]
pub fn Spinner(
    #[props(default)] size: Size,
    #[props(default)] aria_label: Option<String>,
    #[props(default)] class: Option<String>,
) -> Element {
    let custom = class.as_deref();
    let full_class = spinner_classes(size, custom);
    let aria_label = aria_label
        .filter(|label| !label.trim().is_empty())
        .unwrap_or_else(|| "Loading".to_string());

    rsx! {
        span {
            role: "status",
            "aria-label": "{aria_label}",
            class: "{full_class}",
        }
    }
}
