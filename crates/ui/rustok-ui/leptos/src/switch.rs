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
use rustok_ui::{SwitchSize, switch_classes};

/// Switch toggle rendered as an accessible `role="switch"` control.
///
/// Supports:
/// - Controlled mode: via `checked` and `set_checked` / `on_checked_change`.
/// - Uncontrolled mode: via `default_checked` and optional `on_checked_change`.
/// - Form integration: via `name` and optional `value` (renders a hidden input for standard form submissions).
/// - WAI-ARIA & Shadcn/Radix compliance: `role="switch"`, `aria-checked`, `data-state`, `data-slot`.
#[component]
pub fn Switch(
    #[prop(optional)] checked: Option<ReadSignal<bool>>,
    #[prop(optional)] set_checked: Option<WriteSignal<bool>>,
    #[prop(optional)] default_checked: Option<bool>,
    #[prop(optional)] on_checked_change: Option<Callback<bool>>,
    #[prop(default = false)] disabled: bool,
    #[prop(default = false)] required: bool,
    #[prop(default = SwitchSize::Md)] size: SwitchSize,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] name: String,
    #[prop(optional, into)] value: Option<String>,
    #[prop(optional, into)] aria_label: Option<String>,
    #[prop(optional, into)] aria_labelledby: Option<String>,
) -> impl IntoView {
    let id = (!id.trim().is_empty()).then_some(id);
    let name = (!name.trim().is_empty()).then_some(name);
    let aria_label = aria_label.and_then(|a| (!a.trim().is_empty()).then_some(a));
    let aria_labelledby = aria_labelledby.and_then(|a| (!a.trim().is_empty()).then_some(a));
    let custom = (!class.trim().is_empty()).then_some(class.as_str());

    let (internal_checked, set_internal_checked) = signal(default_checked.unwrap_or(false));
    let is_checked = move || {
        if let Some(c) = checked {
            c.get()
        } else {
            internal_checked.get()
        }
    };

    let (track_classes, thumb_classes) = {
        let (track_off, thumb_off) = switch_classes(false, size, custom);
        let (track_on, thumb_on) = switch_classes(true, size, custom);
        (
            move || {
                if is_checked() {
                    track_on.clone()
                } else {
                    track_off.clone()
                }
            },
            move || {
                if is_checked() {
                    thumb_on.clone()
                } else {
                    thumb_off.clone()
                }
            },
        )
    };

    let resolved_value = value
        .and_then(|v| (!v.trim().is_empty()).then_some(v))
        .unwrap_or_else(|| "on".to_string());

    let on_click = move |_| {
        if !disabled {
            let next = !is_checked();
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
    };

    let data_state = move || if is_checked() { "checked" } else { "unchecked" };

    view! {
        <button
            id=id
            type="button"
            role="switch"
            aria-checked=move || is_checked().to_string()
            aria-required=required.then_some("true")
            aria-label=aria_label
            aria-labelledby=aria_labelledby
            disabled=disabled
            data-slot="switch"
            data-state=data_state
            data-disabled=disabled.then_some("")
            class=track_classes
            on:click=on_click
        >
            <span
                data-slot="switch-thumb"
                data-state=data_state
                class=thumb_classes
            />
        </button>
        {name.map(|field_name| {
            view! {
                <input
                    type="checkbox"
                    aria-hidden="true"
                    tabindex="-1"
                    name=field_name
                    value=resolved_value.clone()
                    disabled=disabled
                    required=required
                    prop:checked=move || is_checked()
                    checked=move || is_checked()
                    class="sr-only"
                    style="transform: translateX(-100%); position: absolute; pointer-events: none; opacity: 0; margin: 0px;"
                />
            }
        })}
    }
}
