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
    Orientation, merge_classes, tabs_content_classes, tabs_list_classes, tabs_trigger_classes,
};

/// Tab container.
///
/// The wrapper carries the layout width; the ARIA relationship between
/// [`TabsTrigger`] and [`TabsContent`] is expressed by handing the same id to
/// `panel_id` (trigger) and `id` (content) / `tab_id` (content).
#[component]
pub fn Tabs(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = merge_classes(&["w-full", custom.unwrap_or("")]);

    view! {
        <div class=full_class>
            {children()}
        </div>
    }
}

/// Tab list container with `role="tablist"` and `aria-orientation`.
#[component]
pub fn TabsList(
    #[prop(default = Orientation::Horizontal)] orientation: Orientation,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = tabs_list_classes(orientation, custom);

    view! {
        <div
            role="tablist"
            aria-orientation=orientation.as_str()
            class=full_class
        >
            {children()}
        </div>
    }
}

/// Tab button.
///
/// `active` drives both the styling and `aria-selected`; `id` becomes the tab
/// element id and `panel_id` is rendered as `aria-controls`.
#[component]
pub fn TabsTrigger(
    #[prop(default = false)] active: bool,
    #[prop(default = false)] disabled: bool,
    #[prop(optional)] on_select: Option<Callback<()>>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] panel_id: String,
    children: Children,
) -> impl IntoView {
    let id = (!id.is_empty()).then_some(id);
    let panel_id = (!panel_id.is_empty()).then_some(panel_id);
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = tabs_trigger_classes(active, disabled, custom);

    view! {
        <button
            type="button"
            id=id
            role="tab"
            aria-selected=active.to_string()
            aria-controls=panel_id
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

/// Tab panel.
///
/// `active` toggles the panel visibility (`block`/`hidden`); `id` names the
/// panel and `tab_id` renders `aria-labelledby` pointing back at the trigger.
#[component]
pub fn TabsContent(
    #[prop(default = false)] active: bool,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] tab_id: String,
    children: Children,
) -> impl IntoView {
    let id = (!id.is_empty()).then_some(id);
    let tab_id = (!tab_id.is_empty()).then_some(tab_id);
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = tabs_content_classes(active, custom);

    view! {
        <div
            id=id
            role="tabpanel"
            aria-labelledby=tab_id
            class=full_class
        >
            {children()}
        </div>
    }
}
