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
    #[props(default)] class: Option<String>,
    #[props(default)] aria_label: Option<String>,
    #[props(default)] aria_labelledby: Option<String>,
    children: Element,
) -> Element {
    let custom = class.as_deref();
    let backdrop_class = dialog_backdrop_classes(open);
    let content_class = dialog_content_classes(open, custom);

    rsx! {
        if open {
            div {
                class: "{backdrop_class}",
                onclick: move |_| {
                    if let Some(ref cb) = on_close {
                        cb.call(());
                    }
                },
            }
            div {
                role: "dialog",
                "aria-modal": "true",
                "aria-label": aria_label.as_deref(),
                "aria-labelledby": aria_labelledby.as_deref(),
                tabindex: "-1",
                class: "{content_class}",
                {children}
            }
        }
    }
}

#[component]
pub fn DialogHeader(#[props(default)] class: Option<String>, children: Element) -> Element {
    let custom = class.as_deref();
    rsx! {
        div { class: "{dialog_header_classes(custom)}",
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
    let custom = class.as_deref();
    rsx! {
        h2 {
            id: id.as_deref(),
            class: "{dialog_title_classes(custom)}",
            {children}
        }
    }
}

#[component]
pub fn DialogDescription(#[props(default)] class: Option<String>, children: Element) -> Element {
    let custom = class.as_deref();
    rsx! {
        p { class: "{dialog_description_classes(custom)}",
            {children}
        }
    }
}

#[component]
pub fn DialogFooter(#[props(default)] class: Option<String>, children: Element) -> Element {
    let custom = class.as_deref();
    rsx! {
        div { class: "{dialog_footer_classes(custom)}",
            {children}
        }
    }
}
