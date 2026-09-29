/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use leptos::prelude::*;
use rustok_ui::{Size, spinner_classes};

/// Animated progress indicator.
///
/// The accessible name defaults to `Loading`; pass `aria_label` with a
/// localized string (`aria_label=move || t("ui.loading")`) when the host
/// renders more than one locale.
#[component]
pub fn Spinner(
    #[prop(default = Size::Md)] size: Size,
    #[prop(optional, into)] aria_label: String,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = spinner_classes(size, custom);
    let aria_label = if aria_label.trim().is_empty() {
        "Loading".to_string()
    } else {
        aria_label
    };

    view! {
        <span
            role="status"
            aria-label=aria_label
            class=full_class
        />
    }
}
