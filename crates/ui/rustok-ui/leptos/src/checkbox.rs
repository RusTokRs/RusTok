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

/// Checkbox input with support for controlled and uncontrolled states.
///
/// Supports:
/// - Controlled mode: via `checked` and `set_checked` / `on_checked_change`.
/// - Uncontrolled mode: via `default_checked` and optional `on_checked_change`.
/// - Tri-state: via `indeterminate` DOM property mapping to `aria-checked="mixed"` and `data-state="indeterminate"`.
/// - WAI-ARIA & Shadcn/Radix compliance: `data-slot="checkbox"`, `data-state`, `aria-checked`, `aria-invalid`, `aria-required`.
/// Empty `id`/`name` values are omitted instead of rendered empty.
#[component]
pub fn Checkbox(
    #[prop(optional)] checked: Option<ReadSignal<bool>>,
    #[prop(optional)] set_checked: Option<WriteSignal<bool>>,
    #[prop(optional)] default_checked: Option<bool>,
    #[prop(optional)] on_checked_change: Option<Callback<bool>>,
    #[prop(into, default = false.into())] indeterminate: Signal<bool>,
    #[prop(into, default = false.into())] invalid: Signal<bool>,
    #[prop(default = false)] disabled: bool,
    #[prop(default = false)] required: bool,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] name: String,
    #[prop(optional, into)] value: String,
    #[prop(optional, into)] aria_label: String,
    #[prop(optional, into)] aria_labelledby: String,
    #[prop(optional, into)] aria_describedby: String,
) -> impl IntoView {
    let id = (!id.trim().is_empty()).then_some(id);
    let name = (!name.trim().is_empty()).then_some(name);
    let value = (!value.trim().is_empty()).then_some(value);
    let aria_label = (!aria_label.trim().is_empty()).then_some(aria_label);
    let aria_labelledby = (!aria_labelledby.trim().is_empty()).then_some(aria_labelledby);
    let aria_describedby = (!aria_describedby.trim().is_empty()).then_some(aria_describedby);
    let custom = (!class.trim().is_empty()).then_some(class.as_str());
    let full_class = checkbox_classes(custom);

    let (internal_checked, set_internal_checked) = signal(default_checked.unwrap_or(false));
    let is_checked = move || {
        if let Some(c) = checked {
            c.get()
        } else {
            internal_checked.get()
        }
    };
    let data_state = move || {
        if indeterminate.get() {
            "indeterminate"
        } else if is_checked() {
            "checked"
        } else {
            "unchecked"
        }
    };
    let aria_checked = move || {
        if indeterminate.get() {
            "mixed"
        } else if is_checked() {
            "true"
        } else {
            "false"
        }
    };

    view! {
        <input
            type="checkbox"
            id=id
            name=name
            value=value
            disabled=disabled
            required=required
            class=full_class
            data-slot="checkbox"
            data-state=data_state
            data-disabled=disabled.then_some("")
            data-invalid=move || invalid.get().then_some("")
            aria-checked=aria_checked
            aria-invalid=move || invalid.get().then_some("true")
            aria-required=required.then_some("true")
            aria-label=aria_label
            aria-labelledby=aria_labelledby
            aria-describedby=aria_describedby
            prop:checked=is_checked
            prop:indeterminate=move || indeterminate.get()
            on:change=move |ev| {
                let next = checkbox_checked(&ev);
                if let Some(set) = set_checked {
                    set.set(next);
                }
                if checked.is_none() {
                    set_internal_checked.set(next);
                }
                if let Some(cb) = on_checked_change {
                    cb.run(next);
                }
            }
        />
    }
}
