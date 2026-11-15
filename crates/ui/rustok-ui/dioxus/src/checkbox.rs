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
use rustok_ui::checkbox_classes;

/// Checkbox input.
///
/// Dioxus 0.7 does not expose the `indeterminate` state: HTML defines no such
/// attribute (it is a DOM property only) and `dioxus-html` declares no
/// `indeterminate` constant, so a tri-state checkbox has to be driven from a
/// host-provided script.
#[component]
pub fn Checkbox(
    #[props(default = false)] checked: bool,
    #[props(default = false)] disabled: bool,
    #[props(default = false)] invalid: bool,
    #[props(default = false)] required: bool,
    #[props(default)] value: Option<String>,
    #[props(default)] onchange: Option<EventHandler<FormEvent>>,
    #[props(default)] class: Option<String>,
    #[props(default)] id: Option<String>,
    #[props(default)] name: Option<String>,
    #[props(default)] aria_label: Option<String>,
    #[props(default)] aria_labelledby: Option<String>,
    #[props(default)] aria_describedby: Option<String>,
) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let full_class = checkbox_classes(custom);
    let id = id.as_deref().filter(|s| !s.trim().is_empty());
    let name = name.as_deref().filter(|s| !s.trim().is_empty());
    let value = value.as_deref().filter(|s| !s.trim().is_empty());
    let aria_label = aria_label.as_deref().filter(|s| !s.trim().is_empty());
    let aria_labelledby = aria_labelledby.as_deref().filter(|s| !s.trim().is_empty());
    let aria_describedby = aria_describedby.as_deref().filter(|s| !s.trim().is_empty());

    rsx! {
        input {
            r#type: "checkbox",
            "data-slot": "checkbox",
            "data-state": if checked { "checked" } else { "unchecked" },
            "data-disabled": disabled.then_some(""),
            "data-invalid": invalid.then_some(""),
            "aria-checked": if checked { "true" } else { "false" },
            "aria-invalid": invalid.then_some("true"),
            "aria-required": required.then_some("true"),
            aria_label: aria_label,
            aria_labelledby: aria_labelledby,
            aria_describedby: aria_describedby,
            id: id,
            name: name,
            value: value,
            required: required,
            disabled: disabled,
            class: "{full_class}",
            checked: checked,
            onchange: move |evt| {
                if let Some(ref handler) = onchange {
                    handler.call(evt);
                }
            }
        }
    }
}
