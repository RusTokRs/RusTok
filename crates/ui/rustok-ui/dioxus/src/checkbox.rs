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

#[component]
pub fn Checkbox(
    #[props(default = false)] checked: bool,
    #[props(default = false)] indeterminate: bool,
    #[props(default = false)] disabled: bool,
    #[props(default)] onchange: Option<EventHandler<FormEvent>>,
    #[props(default)] class: Option<String>,
    #[props(default)] id: Option<String>,
    #[props(default)] name: Option<String>,
) -> Element {
    let custom = class.as_deref();
    let full_class = checkbox_classes(disabled, custom);

    rsx! {
        input {
            r#type: "checkbox",
            id: id.as_deref(),
            name: name.as_deref(),
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
