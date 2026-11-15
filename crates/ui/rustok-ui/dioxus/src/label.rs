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
use rustok_ui::label_classes;

#[component]
pub fn Label(
    #[props(default = false)] required: bool,
    #[props(default = false)] disabled: bool,
    #[props(default)] id: Option<String>,
    #[props(default)] r#for: Option<String>,
    #[props(default)] class: Option<String>,
    children: Element,
) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let id = id.as_deref().filter(|s| !s.trim().is_empty());
    let r#for = r#for.as_deref().filter(|s| !s.trim().is_empty());
    let full_class = label_classes(disabled, custom);

    rsx! {
        label {
            id: id,
            r#for: r#for,
            "data-slot": "label",
            "data-disabled": disabled.then_some(""),
            class: "{full_class}",
            {children}
            if required {
                span { class: "text-destructive ml-1", "*" }
            }
        }
    }
}
