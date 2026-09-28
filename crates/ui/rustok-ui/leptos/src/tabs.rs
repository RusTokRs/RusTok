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
use rustok_ui::{Orientation, tabs_content_classes, tabs_list_classes, tabs_trigger_classes};

#[component]
pub fn Tabs(
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    view! {
        <div class=format!("w-full {}", class)>
            {children()}
        </div>
    }
}

#[component]
pub fn TabsList(
    #[prop(default = Orientation::Horizontal)] orientation: Orientation,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = tabs_list_classes(orientation, custom);

    view! {
        <div role="tablist" class=full_class>
            {children()}
        </div>
    }
}

#[component]
pub fn TabsTrigger(
    #[prop(default = false)] active: bool,
    #[prop(default = false)] disabled: bool,
    #[prop(optional)] on_select: Option<Callback<()>>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = tabs_trigger_classes(active, disabled, custom);

    view! {
        <button
            type="button"
            role="tab"
            aria-selected=active.to_string()
            disabled=disabled
            class=full_class
            on:click=move |_| {
                if !disabled && let Some(cb) = on_select {
                    cb.run(());
                }
            }
        >
            {children()}
        </button>
    }
}

#[component]
pub fn TabsContent(
    #[prop(default = false)] active: bool,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = tabs_content_classes(active, custom);

    view! {
        <div
            role="tabpanel"
            class=full_class
        >
            {children()}
        </div>
    }
}
