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
use leptos::html;
use leptos::prelude::*;
use rustok_ui::{
    dialog_backdrop_classes, dialog_content_classes, dialog_description_classes,
    dialog_footer_classes, dialog_header_classes, dialog_title_classes,
};

/// Modal dialog.
///
/// The backdrop and the dialog container are rendered only while `open` is
/// true. Clicking the backdrop or pressing `Escape` calls `on_close`; the
/// container receives focus when it opens and is named through `aria_label`
/// or `aria_labelledby` (the latter usually pointing at a [`DialogTitle`] id).
/// Focus trapping and focus restoration stay with the host application.
#[component]
pub fn Dialog(
    #[prop(default = false)] open: bool,
    #[prop(optional)] on_close: Option<Callback<()>>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] aria_label: String,
    #[prop(optional, into)] aria_labelledby: String,
    children: Children,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let backdrop_class = dialog_backdrop_classes(open);
    let content_class = dialog_content_classes(open, custom);
    let label_attr = (!aria_label.is_empty()).then_some(aria_label);
    let labelledby_attr = (!aria_labelledby.is_empty()).then_some(aria_labelledby);
    let dialog_ref = NodeRef::<html::Div>::new();

    // Move focus into the dialog when it opens so that the modal is reachable
    // for keyboard users and `Escape` reaches the handler below.
    Effect::new(move |_| {
        if open && let Some(element) = dialog_ref.get() {
            let _ = element.focus();
        }
    });

    view! {
        {open.then(|| {
            view! {
                <div
                    class=backdrop_class
                    on:click=move |_| {
                        if let Some(cb) = on_close {
                            cb.run(());
                        }
                    }
                />
                <div
                    node_ref=dialog_ref
                    role="dialog"
                    aria-modal="true"
                    aria-label=label_attr
                    aria-labelledby=labelledby_attr
                    tabindex="-1"
                    class=content_class
                    on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                        if ev.key() == "Escape" && let Some(cb) = on_close {
                            cb.run(());
                        }
                    }
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
