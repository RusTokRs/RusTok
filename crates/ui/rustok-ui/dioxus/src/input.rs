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
use rustok_ui::{Size, input_classes};

#[component]
pub fn Input(
    #[props(default = "text")] r#type: &'static str,
    #[props(default)] size: Size,
    #[props(default = false)] disabled: bool,
    #[props(default = false)] invalid: bool,
    #[props(default)] placeholder: Option<String>,
    #[props(default)] value: Option<String>,
    #[props(default)] oninput: Option<EventHandler<FormEvent>>,
    #[props(default)] prefix: Option<Element>,
    #[props(default)] suffix: Option<Element>,
    #[props(default)] class: Option<String>,
    #[props(default)] id: Option<String>,
    #[props(default)] name: Option<String>,
) -> Element {
    let custom = class.as_deref();
    let has_prefix = prefix.is_some();
    let has_suffix = suffix.is_some();
    let mut full_class = input_classes(size, invalid, custom);
    if has_prefix {
        full_class.push_str(" pl-9");
    }
    if has_suffix {
        full_class.push_str(" pr-9");
    }
    let type_attr = r#type;

    if !has_prefix && !has_suffix {
        rsx! {
            input {
                id: id.as_deref(),
                r#type: "{type_attr}",
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
    } else {
        rsx! {
            div {
                class: "relative flex items-center w-full",
                if let Some(p) = prefix {
                    span {
                        class: "pointer-events-none absolute left-3 flex items-center text-muted-foreground",
                        {p}
                    }
                }
                input {
                    id: id.as_deref(),
                    r#type: "{type_attr}",
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
                if let Some(s) = suffix {
                    span {
                        class: "pointer-events-none absolute right-3 flex items-center text-muted-foreground",
                        {s}
                    }
                }
            }
        }
    }
}
