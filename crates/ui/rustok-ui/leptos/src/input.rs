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
use rustok_ui::{Size, input_classes};

#[component]
pub fn Input(
    #[prop(default = "text")] r#type: &'static str,
    #[prop(default = Size::Md)] size: Size,
    #[prop(default = false)] disabled: bool,
    #[prop(default = false)] invalid: bool,
    #[prop(optional, into)] placeholder: String,
    #[prop(optional)] value: Option<ReadSignal<String>>,
    #[prop(optional)] set_value: Option<WriteSignal<String>>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] name: String,
) -> impl IntoView {
    let id = (!id.is_empty()).then_some(id);
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = input_classes(size, invalid, disabled, custom);

    view! {
        <input
            id=id
            type=r#type
            class=full_class
            disabled=disabled
            aria-invalid=invalid
            placeholder=placeholder
            name=name
            prop:value=move || value.map(|v| v.get()).unwrap_or_default()
            on:input=move |ev| {
                if let Some(set) = set_value {
                    set.set(event_target_value(&ev));
                }
            }
        />
    }
}
