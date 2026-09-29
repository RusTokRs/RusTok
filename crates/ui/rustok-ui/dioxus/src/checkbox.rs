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
/// Dioxus 0.6 does not expose the `indeterminate` state: HTML defines no such
/// attribute (it is a DOM property only) and `dioxus-html` declares no
/// `indeterminate` constant, so a tri-state checkbox has to be driven from a
/// host-provided script.
#[component]
pub fn Checkbox(
    #[props(default = false)] checked: bool,
    #[props(default = false)] disabled: bool,
    #[props(default)] onchange: Option<EventHandler<FormEvent>>,
    #[props(default)] class: Option<String>,
    #[props(default)] id: Option<String>,
    #[props(default)] name: Option<String>,
) -> Element {
    let custom = class.as_deref();
    let full_class = checkbox_classes(custom);

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
