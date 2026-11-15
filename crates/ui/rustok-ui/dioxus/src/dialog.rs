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
    dialog_backdrop_classes, dialog_content_classes, dialog_description_classes,
    dialog_footer_classes, dialog_header_classes, dialog_title_classes,
};

/// Modal dialog. Provide an accessible name with `aria_label` or
/// `aria_labelledby`; the latter should reference a [`DialogTitle`] id.
/// Keyboard focus management and Escape handling remain host responsibilities.
#[component]
pub fn Dialog(
    #[props(default = false)] open: bool,
    #[props(default)] on_close: Option<EventHandler<()>>,
    #[props(default)] id: Option<String>,
    #[props(default)] class: Option<String>,
    #[props(default)] aria_label: Option<String>,
    #[props(default)] aria_labelledby: Option<String>,
    #[props(default)] aria_describedby: Option<String>,
    children: Element,
) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let id = id.as_deref().filter(|s| !s.trim().is_empty());
    let aria_label = aria_label.as_deref().filter(|s| !s.trim().is_empty());
    let aria_labelledby = aria_labelledby.as_deref().filter(|s| !s.trim().is_empty());
    let aria_describedby = aria_describedby.as_deref().filter(|s| !s.trim().is_empty());
    let backdrop_class = dialog_backdrop_classes(open);
    let content_class = dialog_content_classes(open, custom);

    rsx! {
        if open {
            div {
                "data-slot": "dialog-overlay",
                "data-state": if open { "open" } else { "closed" },
                class: "{backdrop_class}",
                onclick: move |_| {
                    if let Some(ref cb) = on_close {
                        cb.call(());
                    }
                },
            }
            div {
                id: id,
                role: "dialog",
                "data-slot": "dialog-content",
                "data-state": if open { "open" } else { "closed" },
                "aria-modal": "true",
                aria_label: aria_label,
                aria_labelledby: aria_labelledby,
                aria_describedby: aria_describedby,
                tabindex: "-1",
                class: "{content_class}",
                {children}
            }
        }
    }
}

#[component]
pub fn DialogHeader(#[props(default)] class: Option<String>, children: Element) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    rsx! {
        div {
            "data-slot": "dialog-header",
            class: "{dialog_header_classes(custom)}",
            {children}
        }
    }
}

/// Heading for a dialog; set `id` when the parent uses `aria_labelledby`.
#[component]
pub fn DialogTitle(
    #[props(default)] id: Option<String>,
    #[props(default)] class: Option<String>,
    children: Element,
) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let id = id.as_deref().filter(|s| !s.trim().is_empty());
    rsx! {
        h2 {
            id: id,
            "data-slot": "dialog-title",
            class: "{dialog_title_classes(custom)}",
            {children}
        }
    }
}

#[component]
pub fn DialogDescription(
    #[props(default)] id: Option<String>,
    #[props(default)] class: Option<String>,
    children: Element,
) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let id = id.as_deref().filter(|s| !s.trim().is_empty());
    rsx! {
        p {
            id: id,
            "data-slot": "dialog-description",
            class: "{dialog_description_classes(custom)}",
            {children}
        }
    }
}

#[component]
pub fn DialogFooter(#[props(default)] class: Option<String>, children: Element) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    rsx! {
        div {
            "data-slot": "dialog-footer",
            class: "{dialog_footer_classes(custom)}",
            {children}
        }
    }
}
