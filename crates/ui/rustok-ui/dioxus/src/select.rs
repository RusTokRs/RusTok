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
use rustok_ui::{SelectOption, Size, select_classes};

#[component]
pub fn Select(
    #[props(default)] size: Size,
    #[props(default = false)] disabled: bool,
    #[props(default = false)] invalid: bool,
    options: Vec<SelectOption>,
    #[props(default)] placeholder: Option<String>,
    #[props(default)] value: Option<String>,
    #[props(default)] onchange: Option<EventHandler<FormEvent>>,
    #[props(default)] class: Option<String>,
    #[props(default)] id: Option<String>,
    #[props(default)] name: Option<String>,
) -> Element {
    let custom = class.as_deref();
    let full_class = select_classes(size, invalid, custom);

    rsx! {
        select {
            id: id.as_deref(),
            class: "{full_class}",
            disabled: disabled,
            "aria-invalid": invalid.to_string(),
            name: name.as_deref(),
            value: value.as_deref(),
            onchange: move |evt| {
                if let Some(ref handler) = onchange {
                    handler.call(evt);
                }
            },
            if let Some(ph) = placeholder {
                option {
                    value: "",
                    disabled: true,
                    selected: value.as_deref().unwrap_or("").is_empty(),
                    "{ph}"
                }
            }
            for opt in options {
                option {
                    key: "{opt.value}",
                    value: "{opt.value}",
                    disabled: opt.disabled,
                    selected: value.as_deref() == Some(&opt.value),
                    "{opt.label}"
                }
            }
        }
    }
}
