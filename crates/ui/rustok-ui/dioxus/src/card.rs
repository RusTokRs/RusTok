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
    CardVariant, card_action_classes, card_classes, card_content_classes, card_description_classes,
    card_footer_classes, card_header_classes, card_title_classes,
};

#[component]
pub fn Card(
    #[props(default)] variant: CardVariant,
    #[props(default)] id: Option<String>,
    #[props(default)] class: Option<String>,
    children: Element,
) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let id = id.as_deref().filter(|s| !s.trim().is_empty());
    let full_class = card_classes(variant, custom);

    rsx! {
        div {
            id: id,
            "data-slot": "card",
            class: "{full_class}",
            {children}
        }
    }
}

#[component]
pub fn CardHeader(#[props(default)] class: Option<String>, children: Element) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let full_class = card_header_classes(custom);

    rsx! {
        div {
            "data-slot": "card-header",
            class: "{full_class}",
            {children}
        }
    }
}

#[component]
pub fn CardTitle(
    #[props(default)] id: Option<String>,
    #[props(default)] class: Option<String>,
    children: Element,
) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let id = id.as_deref().filter(|s| !s.trim().is_empty());
    let full_class = card_title_classes(custom);

    rsx! {
        div {
            id: id,
            "data-slot": "card-title",
            class: "{full_class}",
            {children}
        }
    }
}

#[component]
pub fn CardDescription(
    #[props(default)] id: Option<String>,
    #[props(default)] class: Option<String>,
    children: Element,
) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let id = id.as_deref().filter(|s| !s.trim().is_empty());
    let full_class = card_description_classes(custom);

    rsx! {
        div {
            id: id,
            "data-slot": "card-description",
            class: "{full_class}",
            {children}
        }
    }
}

#[component]
pub fn CardAction(#[props(default)] class: Option<String>, children: Element) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let full_class = card_action_classes(custom);

    rsx! {
        div {
            "data-slot": "card-action",
            class: "{full_class}",
            {children}
        }
    }
}

#[component]
pub fn CardContent(#[props(default)] class: Option<String>, children: Element) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let full_class = card_content_classes(custom);

    rsx! {
        div {
            "data-slot": "card-content",
            class: "{full_class}",
            {children}
        }
    }
}

#[component]
pub fn CardFooter(#[props(default)] class: Option<String>, children: Element) -> Element {
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let full_class = card_footer_classes(custom);

    rsx! {
        div {
            "data-slot": "card-footer",
            class: "{full_class}",
            {children}
        }
    }
}
