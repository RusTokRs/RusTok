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
    #[prop(into, default = false.into())] disabled: Signal<bool>,
    #[prop(into, default = false.into())] invalid: Signal<bool>,
    #[prop(default = false)] required: bool,
    #[prop(optional, into)] placeholder: String,
    #[prop(optional)] value: Option<ReadSignal<String>>,
    #[prop(optional, into)] default_value: Option<String>,
    /// A value derived from other signals, for a field whose text is the resolved response with the
    /// operator's edits laid over it. `value` stays `ReadSignal` because that is what an owned cell
    /// is; a derived value has no cell to hand over, and a `ReadSignal` cannot be built from a
    /// closure.
    #[prop(optional)]
    derived_value: Option<Signal<String>>,
    /// The setter for the field. A form owns a `WriteSignal`; a caller writing into a projection
    /// passes `SignalSetter::map(..)`, which is the type Leptos has for "any setter" — the write
    /// face of a value that is not an owned cell.
    #[prop(optional, into)]
    set_value: Option<SignalSetter<String>>,
    /// Callback invoked with the new input string upon user input.
    #[prop(optional)]
    on_change: Option<Callback<String>>,
    #[prop(optional, into)] prefix: Option<AnyView>,
    #[prop(optional, into)] suffix: Option<AnyView>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] name: String,
    #[prop(optional, into)] described_by: String,
    #[prop(optional, into)] input_mode: String,
    #[prop(optional, into)] aria_label: String,
    #[prop(optional, into)] aria_labelledby: String,
) -> impl IntoView {
    // A field is either an owned cell or a value derived from the response; the derived one wins
    // when given, so a caller migrating a form off an `Effect` does not have to invent a cell.
    // If neither is passed, an internal signal initialized with `default_value` drives the field.
    let (internal_value, set_internal_value) = signal(default_value.unwrap_or_default());
    let current_value = move || {
        derived_value
            .map(|derived| derived.get())
            .or_else(|| value.map(|cell| cell.get()))
            .unwrap_or_else(|| internal_value.get())
    };
    let id = (!id.trim().is_empty()).then_some(id);
    let name = (!name.trim().is_empty()).then_some(name);
    let described_by = (!described_by.trim().is_empty()).then_some(described_by);
    let input_mode = (!input_mode.trim().is_empty()).then_some(input_mode);
    let aria_label = (!aria_label.trim().is_empty()).then_some(aria_label);
    let aria_labelledby = (!aria_labelledby.trim().is_empty()).then_some(aria_labelledby);
    let custom_class = StoredValue::new(class);
    let custom = move || custom_class.get_value();

    if prefix.is_none() && suffix.is_none() {
        let full_class = move || {
            let class = custom();
            input_classes(
                size,
                invalid.get(),
                (!class.trim().is_empty()).then_some(class.as_str()),
            )
        };
        view! {
            <input
                id=id
                type=r#type
                data-slot="input"
                class=full_class
                disabled=move || disabled.get()
                required=required
                aria-required=required.then_some("true")
                aria-invalid=move || invalid.get()
                data-invalid=move || invalid.get().then_some("")
                data-disabled=move || disabled.get().then_some("")
                aria-describedby=described_by
                aria-label=aria_label.clone()
                aria-labelledby=aria_labelledby.clone()
                inputmode=input_mode
                placeholder=placeholder
                name=name
                prop:value=move || current_value()
                on:input=move |ev| {
                    let next = event_target_value(&ev);
                    if let Some(set) = set_value {
                        set.set(next.clone());
                    }
                    if value.is_none() && derived_value.is_none() {
                        set_internal_value.set(next.clone());
                    }
                    if let Some(cb) = on_change {
                        cb.run(next);
                    }
                }
            />
        }
        .into_any()
    } else {
        let has_prefix = prefix.is_some();
        let has_suffix = suffix.is_some();
        let full_class = move || {
            let class = custom();
            let mut full_class = input_classes(
                size,
                invalid.get(),
                (!class.trim().is_empty()).then_some(class.as_str()),
            );
            if has_prefix {
                full_class.push_str(" pl-9");
            }
            if has_suffix {
                full_class.push_str(" pr-9");
            }
            full_class
        };
        view! {
            <div data-slot="input-wrapper" class="relative flex items-center w-full">
                {prefix.map(|p| view! {
                    <span class="pointer-events-none absolute left-3 flex items-center text-muted-foreground">
                        {p}
                    </span>
                })}
                <input
                    id=id
                    type=r#type
                    data-slot="input"
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
                    inputmode=input_mode
                    placeholder=placeholder
                    name=name
                    prop:value=move || current_value()
                    on:input=move |ev| {
                        let next = event_target_value(&ev);
                        if let Some(set) = set_value {
                            set.set(next.clone());
                        }
                        if value.is_none() && derived_value.is_none() {
                            set_internal_value.set(next.clone());
                        }
                        if let Some(cb) = on_change {
                            cb.run(next);
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
