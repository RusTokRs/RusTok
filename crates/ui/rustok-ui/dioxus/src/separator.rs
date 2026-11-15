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
use rustok_ui::{Orientation, separator_classes};

#[component]
pub fn Separator(
    #[props(default)] orientation: Orientation,
    #[props(default = true)] decorative: bool,
    #[props(default)] id: Option<String>,
    #[props(default)] class: Option<String>,
) -> Element {
    let id = id.as_deref().filter(|s| !s.trim().is_empty());
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let full_class = separator_classes(orientation, custom);

    rsx! {
        div {
            id: id,
            role: (!decorative).then_some("separator"),
            "data-slot": "separator",
            "data-orientation": orientation.as_str(),
            "aria-orientation": (!decorative).then_some(orientation.as_str()),
            "aria-hidden": decorative.then_some("true"),
            class: "{full_class}",
        }
    }
}
