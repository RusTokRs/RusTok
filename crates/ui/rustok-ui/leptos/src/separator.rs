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

/// Horizontal or vertical divider with `data-slot="separator"`,
/// `data-orientation`, and accessible role semantics.
///
/// When `decorative` is true (the default), `aria-hidden="true"` is set to avoid
/// cluttering screen readers. When false, `role="separator"` and `aria-orientation`
/// are rendered.
#[component]
pub fn Separator(
    #[prop(default = Orientation::Horizontal)] orientation: Orientation,
    #[prop(default = true)] decorative: bool,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let id = (!id.trim().is_empty()).then_some(id);
    let custom = (!class.trim().is_empty()).then_some(class.as_str());
    let full_class = separator_classes(orientation, custom);

    view! {
        <div
            id=id
            data-slot="separator"
            data-orientation=orientation.as_str()
            class=full_class
            role=move || (!decorative).then_some("separator")
            aria-orientation=move || (!decorative).then_some(orientation.as_str())
            aria-hidden=move || decorative.then_some("true")
        />
    }
}
