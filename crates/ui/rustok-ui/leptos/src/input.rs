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
    #[prop(optional, into)] placeholder: String,
    #[prop(optional)] value: Option<ReadSignal<String>>,
    /// A value derived from other signals, for a field whose text is the resolved response with the
    /// operator's edits laid over it. `value` stays `ReadSignal` because that is what an owned cell
    /// is; a derived value has no cell to hand over, and a `ReadSignal` cannot be built from a
    /// closure.
    #[prop(optional)] derived_value: Option<Signal<String>>,
    /// The setter for the field. A form owns a `WriteSignal`; a caller writing into a projection
    /// passes `SignalSetter::map(..)`, which is the type Leptos has for "any setter" — the write
    /// face of a value that is not an owned cell.
    #[prop(optional, into)] set_value: Option<SignalSetter<String>>,
    #[prop(optional, into)] prefix: Option<AnyView>,
    #[prop(optional, into)] suffix: Option<AnyView>,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] name: String,
    #[prop(optional, into)] described_by: String,
    #[prop(optional, into)] input_mode: String,
) -> impl IntoView {
    // A field is either an owned cell or a value derived from the response; the derived one wins
    // when given, so a caller migrating a form off an `Effect` does not have to invent a cell.
    let current_value = move || {
        derived_value
            .map(|derived| derived.get())
            .or_else(|| value.map(|cell| cell.get()))
            .unwrap_or_default()
    };
    let id = (!id.is_empty()).then_some(id);
    let name = (!name.is_empty()).then_some(name);
    let described_by = (!described_by.is_empty()).then_some(described_by);
    let input_mode = (!input_mode.is_empty()).then_some(input_mode);
    let custom_class = StoredValue::new(class);
    let custom = move || custom_class.get_value();

    if prefix.is_none() && suffix.is_none() {
        let full_class = move || {
            let class = custom();
            input_classes(
                size,
                invalid.get(),
                (!class.is_empty()).then_some(class.as_str()),
            )
        };
        view! {
            <input
                id=id
                type=r#type
                class=full_class
                disabled=move || disabled.get()
                aria-invalid=move || invalid.get()
                aria-describedby=described_by
                inputmode=input_mode
                placeholder=placeholder
                name=name
                prop:value=move || current_value()
                on:input=move |ev| {
                    if let Some(set) = set_value {
                        set.set(event_target_value(&ev));
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
                (!class.is_empty()).then_some(class.as_str()),
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
                    disabled=move || disabled.get()
                    aria-invalid=move || invalid.get()
                aria-describedby=described_by
                inputmode=input_mode
                    placeholder=placeholder
                    name=name
                    prop:value=move || current_value()
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
