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
use rustok_ui::{Orientation, separator_classes};

/// Horizontal or vertical divider with `role="separator"` and the matching
/// `aria-orientation`.
#[component]
pub fn Separator(
    #[prop(default = Orientation::Horizontal)] orientation: Orientation,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = separator_classes(orientation, custom);

    view! {
        <div
            class=full_class
            role="separator"
            aria-orientation=orientation.as_str()
        />
    }
}
