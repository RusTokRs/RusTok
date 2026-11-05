/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use leptos::children::Children;
use leptos::prelude::*;
use rustok_ui::{
    CardVariant, card_action_classes, card_classes, card_content_classes, card_description_classes,
    card_footer_classes, card_header_classes, card_title_classes,
};

#[component]
pub fn Card(
    #[prop(default = CardVariant::Default)] variant: CardVariant,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = card_classes(variant, custom);

    view! {
        <div class=full_class>
            {children()}
        </div>
    }
}

#[component]
pub fn CardHeader(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <div class=card_header_classes(custom)>
            {children()}
        </div>
    }
}

#[component]
pub fn CardTitle(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <div class=card_title_classes(custom)>
            {children()}
        </div>
    }
}

#[component]
pub fn CardDescription(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <div class=card_description_classes(custom)>
            {children()}
        </div>
    }
}

#[component]
pub fn CardAction(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <div class=card_action_classes(custom)>
            {children()}
        </div>
    }
}

#[component]
pub fn CardContent(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <div class=card_content_classes(custom)>
            {children()}
        </div>
    }
}

#[component]
pub fn CardFooter(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <div class=card_footer_classes(custom)>
            {children()}
        </div>
    }
}
