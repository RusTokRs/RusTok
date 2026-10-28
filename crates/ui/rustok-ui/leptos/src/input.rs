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

/// Single-line text control.
///
/// `r#type` forwards the native HTML type (`text`, `email`, `date`, …), while
/// [`rustok_ui::InputType`] stays available as the typed contract for hosts
/// that prefer an enum. `invalid` renders `aria-invalid` and switches the
/// border/ring palette to the destructive one. Empty `id`/`name` values are
/// omitted instead of rendered as empty attributes.
#[component]
pub fn Input(
    #[prop(default = "text")] r#type: &'static str,
    #[prop(default = Size::Md)] size: Size,
    #[prop(default = false)] disabled: bool,
    #[prop(default = false)] invalid: bool,
    #[prop(optional, into)] placeholder: String,
    #[prop(optional)] value: Option<ReadSignal<String>>,
    #[prop(optional)] set_value: Option<WriteSignal<String>>,
    #[prop(optional, into)] prefix: Option<AnyView>,
    #[prop(optional, into)] suffix: Option<AnyView>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] name: String,
) -> impl IntoView {
    let id = (!id.is_empty()).then_some(id);
    let name = (!name.is_empty()).then_some(name);
    let custom = (!class.is_empty()).then_some(class.as_str());

    if prefix.is_none() && suffix.is_none() {
        let full_class = input_classes(size, invalid, custom);
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
        .into_any()
    } else {
        let mut full_class = input_classes(size, invalid, custom);
        if prefix.is_some() {
            full_class.push_str(" pl-9");
        }
        if suffix.is_some() {
            full_class.push_str(" pr-9");
        }
        view! {
            <div class="relative flex items-center w-full">
                {prefix.map(|p| view! {
                    <span class="pointer-events-none absolute left-3 flex items-center text-muted-foreground">
                        {p}
                    </span>
                })}
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
                {suffix.map(|s| view! {
                    <span class="pointer-events-none absolute right-3 flex items-center text-muted-foreground">
                        {s}
                    </span>
                })}
            </div>
        }
        .into_any()
    }
}
