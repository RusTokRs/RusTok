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
    dialog_backdrop_classes, dialog_content_classes, dialog_description_classes,
    dialog_footer_classes, dialog_header_classes, dialog_title_classes,
};

#[component]
pub fn Dialog(
    #[prop(default = false)] open: bool,
    #[prop(optional)] on_close: Option<Callback<()>>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let backdrop_class = dialog_backdrop_classes(open);
    let content_class = dialog_content_classes(open, custom);

    view! {
        {open.then(|| {
            view! {
                <div
                    class=backdrop_class
                    aria-hidden="true"
                    on:click=move |_| {
                        if let Some(cb) = on_close {
                            cb.run(());
                        }
                    }
                />
                <div
                    role="dialog"
                    aria-modal="true"
                    class=content_class
                >
                    {children()}
                </div>
            }
        })}
    }
}

#[component]
pub fn DialogHeader(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <div class=dialog_header_classes(custom)>
            {children()}
        </div>
    }
}

#[component]
pub fn DialogTitle(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <h2 class=dialog_title_classes(custom)>
            {children()}
        </h2>
    }
}

#[component]
pub fn DialogDescription(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <p class=dialog_description_classes(custom)>
            {children()}
        </p>
    }
}

#[component]
pub fn DialogFooter(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    view! {
        <div class=dialog_footer_classes(custom)>
            {children()}
        </div>
    }
}
