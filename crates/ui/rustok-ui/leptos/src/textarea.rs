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
/// Supports:
/// - Controlled mode: via `value` or `derived_value` and `set_value`.
/// - Uncontrolled mode: via `default_value` and `on_input`.
/// - WAI-ARIA & Shadcn/Radix compliance: `data-slot="textarea"`, `data-disabled`, `data-invalid`,
///   `aria-invalid`, `aria-required`, `aria-describedby`, `aria-label`, `aria-labelledby`.
/// `rows` defaults to three; empty `id`/`name` values are omitted.
#[component]
pub fn Textarea(
    #[prop(default = Size::Md)] size: Size,
    #[prop(into, default = false.into())] disabled: Signal<bool>,
    #[prop(into, default = false.into())] invalid: Signal<bool>,
    #[prop(default = false)] required: bool,
    #[prop(default = 3u32)] rows: u32,
    #[prop(optional, into)] placeholder: String,
    #[prop(optional)] value: Option<ReadSignal<String>>,
    #[prop(optional, into)] default_value: Option<String>,
    #[prop(optional)] derived_value: Option<Signal<String>>,
    #[prop(optional, into)] set_value: Option<SignalSetter<String>>,
    #[prop(optional, into)] on_input: Option<Callback<String>>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] name: String,
    #[prop(optional, into)] described_by: String,
    #[prop(optional, into)] aria_label: String,
    #[prop(optional, into)] aria_labelledby: String,
) -> impl IntoView {
    let (internal_value, set_internal_value) = signal(default_value.unwrap_or_default());
    let current_value = move || {
        derived_value
            .map(|d| d.get())
            .or_else(|| value.map(|v| v.get()))
            .unwrap_or_else(|| internal_value.get())
    };
    let id = (!id.trim().is_empty()).then_some(id);
    let name = (!name.trim().is_empty()).then_some(name);
    let described_by = (!described_by.trim().is_empty()).then_some(described_by);
    let aria_label = (!aria_label.trim().is_empty()).then_some(aria_label);
    let aria_labelledby = (!aria_labelledby.trim().is_empty()).then_some(aria_labelledby);
    let custom = StoredValue::new(class);

    let full_class = move || {
        let c = custom.get_value();
        textarea_classes(size, invalid.get(), (!c.trim().is_empty()).then_some(c.as_str()))
    };

    view! {
        <textarea
            id=id
            data-slot="textarea"
            rows=rows
            class=full_class
            disabled=move || disabled.get()
            required=required
            aria-required=required.then_some("true")
            aria-invalid=move || invalid.get()
            data-invalid=move || invalid.get().then_some("")
            data-disabled=move || disabled.get().then_some("")
            aria-describedby=described_by
            aria-label=aria_label
            aria-labelledby=aria_labelledby
            placeholder=placeholder
            name=name
            prop:value=current_value
            on:input=move |ev| {
                if !disabled.get_untracked() {
                    let val = event_target_value(&ev);
                    if let Some(ref set) = set_value {
                        set.set(val.clone());
                    }
                    if value.is_none() && derived_value.is_none() {
                        set_internal_value.set(val.clone());
                    }
                    if let Some(cb) = on_input {
                        cb.run(val);
                    }
                }
            }
        />
    }
}
