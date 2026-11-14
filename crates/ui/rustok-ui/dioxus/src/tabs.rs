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
use rustok_ui::{
    Orientation, merge_classes, tabs_content_classes, tabs_list_classes, tabs_trigger_classes,
};

#[component]
pub fn Tabs(#[props(default)] class: Option<String>, children: Element) -> Element {
    let custom = class.as_deref();
    let full_class = merge_classes(&["w-full", custom.unwrap_or("")]);

    rsx! {
        div {
            "data-slot": "tabs",
            class: "{full_class}",
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
        div {
            role: "tablist",
            "data-slot": "tabs-list",
            "data-orientation": orientation.as_str(),
            "aria-orientation": orientation.as_str(),
            class: "{full_class}",
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
            "data-slot": "tabs-trigger",
            "data-state": if active { "active" } else { "inactive" },
            "data-disabled": disabled.then_some(""),
            "aria-selected": active.to_string(),
            tabindex: if active { "0" } else { "-1" },
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
            "data-slot": "tabs-content",
            "data-state": if active { "active" } else { "inactive" },
            class: "{full_class}",
            {children}
        }
    }
}
