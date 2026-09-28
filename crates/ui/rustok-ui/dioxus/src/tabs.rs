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
use rustok_ui::{Orientation, tabs_content_classes, tabs_list_classes, tabs_trigger_classes};

#[component]
pub fn Tabs(#[props(default)] class: Option<String>, children: Element) -> Element {
    rsx! {
        div { class: "w-full {class.as_deref().unwrap_or(\"\")}",
            {children}
        }
    }
}

#[component]
pub fn TabsList(
    #[props(default)] orientation: Orientation,
    #[props(default)] class: Option<String>,
    children: Element,
) -> Element {
    let custom = class.as_deref();
    let full_class = tabs_list_classes(orientation, custom);

    rsx! {
        div { role: "tablist", class: "{full_class}",
            {children}
        }
    }
}

#[component]
pub fn TabsTrigger(
    #[props(default = false)] active: bool,
    #[props(default = false)] disabled: bool,
    #[props(default)] onselect: Option<EventHandler<()>>,
    #[props(default)] class: Option<String>,
    children: Element,
) -> Element {
    let custom = class.as_deref();
    let full_class = tabs_trigger_classes(active, disabled, custom);

    rsx! {
        button {
            r#type: "button",
            role: "tab",
            "aria-selected": active.to_string(),
            disabled: disabled,
            class: "{full_class}",
            onclick: move |_| {
                if !disabled && let Some(ref cb) = onselect {
                    cb.call(());
                }
            },
            {children}
        }
    }
}

#[component]
pub fn TabsContent(
    #[props(default = false)] active: bool,
    #[props(default)] class: Option<String>,
    children: Element,
) -> Element {
    let custom = class.as_deref();
    let full_class = tabs_content_classes(active, custom);

    rsx! {
        div {
            role: "tabpanel",
            class: "{full_class}",
            {children}
        }
    }
}
