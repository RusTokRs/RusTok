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
use rustok_ui::{SelectOption, Size, select_classes};

#[component]
pub fn Select(
    #[prop(default = Size::Md)] size: Size,
    #[prop(default = false)] disabled: bool,
    #[prop(default = false)] invalid: bool,
    options: Vec<SelectOption>,
    #[prop(optional, into)] placeholder: String,
    #[prop(optional)] value: Option<ReadSignal<String>>,
    #[prop(optional)] set_value: Option<WriteSignal<String>>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] name: String,
) -> impl IntoView {
    let id = (!id.is_empty()).then_some(id);
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = select_classes(size, invalid, disabled, custom);

    view! {
        <select
            id=id
            class=full_class
            disabled=disabled
            aria-invalid=invalid
            name=name
            on:change=move |ev| {
                if let Some(set) = set_value {
                    set.set(event_target_value(&ev));
                }
            }
        >
            {(!placeholder.is_empty()).then(|| view! {
                <option value="" disabled=true selected=move || value.map(|v| v.get()).unwrap_or_default().is_empty()>
                    {placeholder.clone()}
                </option>
            })}
            {options.into_iter().map(|opt| {
                let opt_value = opt.value;
                let opt_val_cmp = opt_value.clone();
                view! {
                    <option
                        value=opt_value
                        disabled=opt.disabled
                        selected=move || value.map(|v| v.get()).unwrap_or_default() == opt_val_cmp
                    >
                        {opt.label}
                    </option>
                }
            }).collect_view()}
        </select>
    }
}
