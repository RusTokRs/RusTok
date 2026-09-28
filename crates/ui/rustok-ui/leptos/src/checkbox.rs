/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use leptos::prelude::*;
use rustok_ui::checkbox_classes;

fn checkbox_checked(ev: &leptos::ev::Event) -> bool {
    use leptos::wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<leptos::web_sys::HtmlInputElement>().ok())
        .map(|el| el.checked())
        .unwrap_or(false)
}

#[component]
pub fn Checkbox(
    #[prop(optional)] checked: Option<ReadSignal<bool>>,
    #[prop(optional)] set_checked: Option<WriteSignal<bool>>,
    #[prop(default = false)] indeterminate: bool,
    #[prop(default = false)] disabled: bool,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] name: String,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = checkbox_classes(disabled, custom);

    view! {
        <input
            type="checkbox"
            id=id
            name=name
            disabled=disabled
            class=full_class
            prop:checked=move || checked.map(|c| c.get()).unwrap_or(false)
            prop:indeterminate=indeterminate
            on:change=move |ev| {
                if let Some(set) = set_checked {
                    set.set(checkbox_checked(&ev));
                }
            }
        />
    }
}
