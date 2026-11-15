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
use rustok_ui::{SwitchSize, switch_classes};

#[component]
pub fn Switch(
    #[props(default = false)] checked: bool,
    #[props(default = false)] disabled: bool,
    #[props(default)] size: SwitchSize,
    #[props(default)] ontoggle: Option<EventHandler<bool>>,
    #[props(default)] class: Option<String>,
    #[props(default)] id: Option<String>,
    #[props(default)] name: Option<String>,
    #[props(default)] value: Option<String>,
    #[props(default)] aria_label: Option<String>,
    #[props(default)] aria_labelledby: Option<String>,
) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let id = id.as_deref().filter(|s| !s.trim().is_empty());
    let name = name.as_deref().filter(|s| !s.trim().is_empty());
    let value = value.as_deref().filter(|s| !s.trim().is_empty());
    let aria_label = aria_label.as_deref().filter(|s| !s.trim().is_empty());
    let aria_labelledby = aria_labelledby.as_deref().filter(|s| !s.trim().is_empty());
    let (track_class, thumb_class) = switch_classes(checked, size, custom);

    rsx! {
        button {
            id: id,
            r#type: "button",
            role: "switch",
            "data-slot": "switch",
            "data-state": if checked { "checked" } else { "unchecked" },
            "data-disabled": disabled.then_some(""),
            "aria-checked": checked.to_string(),
            aria_label: aria_label,
            aria_labelledby: aria_labelledby,
            name: name,
            value: value,
            disabled: disabled,
            class: "{track_class}",
            onclick: move |_| {
                if !disabled && let Some(ref cb) = ontoggle {
                    cb.call(!checked);
                }
            },
            span {
                "data-slot": "switch-thumb",
                "data-state": if checked { "checked" } else { "unchecked" },
                class: "{thumb_class}"
            }
        }
    }
}
