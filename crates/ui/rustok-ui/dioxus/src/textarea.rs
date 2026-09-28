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
    #[props(default = 3u32)] rows: u32,
    #[props(default)] placeholder: Option<String>,
    #[props(default)] value: Option<String>,
    #[props(default)] oninput: Option<EventHandler<FormEvent>>,
    #[props(default)] class: Option<String>,
    #[props(default)] id: Option<String>,
    #[props(default)] name: Option<String>,
) -> Element {
    let custom = class.as_deref();
    let full_class = textarea_classes(size, invalid, disabled, custom);

    rsx! {
        textarea {
            id: id.as_deref(),
            rows: "{rows}",
            class: "{full_class}",
            disabled: disabled,
            "aria-invalid": invalid.to_string(),
            placeholder: placeholder.as_deref(),
            name: name.as_deref(),
            value: value.as_deref().unwrap_or(""),
            oninput: move |evt| {
                if let Some(ref handler) = oninput {
                    handler.call(evt);
                }
            }
        }
    }
}
