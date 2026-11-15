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
use rustok_ui::{Size, textarea_classes};

#[component]
pub fn Textarea(
    #[props(default)] size: Size,
    #[props(default = false)] disabled: bool,
    #[props(default = false)] invalid: bool,
    #[props(default = false)] required: bool,
    #[props(default = 3u32)] rows: u32,
    #[props(default)] placeholder: Option<String>,
    #[props(default)] value: Option<String>,
    #[props(default)] oninput: Option<EventHandler<FormEvent>>,
    #[props(default)] class: Option<String>,
    #[props(default)] id: Option<String>,
    #[props(default)] name: Option<String>,
    #[props(default)] aria_label: Option<String>,
    #[props(default)] aria_labelledby: Option<String>,
    #[props(default)] aria_describedby: Option<String>,
) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let id = id.as_deref().filter(|s| !s.trim().is_empty());
    let name = name.as_deref().filter(|s| !s.trim().is_empty());
    let aria_label = aria_label.as_deref().filter(|s| !s.trim().is_empty());
    let aria_labelledby = aria_labelledby.as_deref().filter(|s| !s.trim().is_empty());
    let aria_describedby = aria_describedby.as_deref().filter(|s| !s.trim().is_empty());
    let full_class = textarea_classes(size, invalid, custom);

    rsx! {
        textarea {
            id: id,
            "data-slot": "textarea",
            "data-disabled": disabled.then_some(""),
            "data-invalid": invalid.then_some(""),
            rows: "{rows}",
            class: "{full_class}",
            disabled: disabled,
            required: required,
            "aria-required": required.then_some("true"),
            "aria-invalid": invalid.to_string(),
            aria_label: aria_label,
            aria_labelledby: aria_labelledby,
            aria_describedby: aria_describedby,
            placeholder: placeholder.as_deref(),
            name: name,
            value: value.as_deref().unwrap_or(""),
            oninput: move |evt| {
                if let Some(ref handler) = oninput {
                    handler.call(evt);
                }
            }
        }
    }
}
