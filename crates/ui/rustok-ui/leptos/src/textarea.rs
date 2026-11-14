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
use rustok_ui::{Size, textarea_classes};

/// Multiline text control with `data-slot="textarea"`, reactive validation,
/// and accessible description wiring.
///
/// `rows` defaults to three; `invalid` renders `aria-invalid` and the
/// destructive border/ring palette. Empty `id`/`name` values are omitted.
#[component]
pub fn Textarea(
    #[prop(default = Size::Md)] size: Size,
    #[prop(into, default = false.into())] disabled: Signal<bool>,
    #[prop(into, default = false.into())] invalid: Signal<bool>,
    #[prop(default = 3u32)] rows: u32,
    #[prop(optional, into)] placeholder: String,
    #[prop(optional)] value: Option<ReadSignal<String>>,
    #[prop(optional)] derived_value: Option<Signal<String>>,
    #[prop(optional, into)] set_value: Option<SignalSetter<String>>,
    #[prop(optional, into)] on_input: Option<Callback<String>>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] name: String,
    #[prop(optional, into)] described_by: String,
) -> impl IntoView {
    let current_value = move || {
        derived_value
            .map(|d| d.get())
            .or_else(|| value.map(|v| v.get()))
            .unwrap_or_default()
    };
    let id = (!id.is_empty()).then_some(id);
    let name = (!name.is_empty()).then_some(name);
    let described_by = (!described_by.is_empty()).then_some(described_by);
    let custom = StoredValue::new(class);

    let full_class = move || {
        let c = custom.get_value();
        textarea_classes(size, invalid.get(), (!c.is_empty()).then_some(c.as_str()))
    };

    view! {
        <textarea
            id=id
            data-slot="textarea"
            rows=rows
            class=full_class
            disabled=move || disabled.get()
            aria-invalid=move || invalid.get().to_string()
            aria-describedby=described_by
            placeholder=placeholder
            name=name
            prop:value=current_value
            on:input=move |ev| {
                let val = event_target_value(&ev);
                if let Some(ref set) = set_value {
                    set.set(val.clone());
                }
                if let Some(cb) = on_input {
                    cb.run(val);
                }
            }
        />
    }
}
