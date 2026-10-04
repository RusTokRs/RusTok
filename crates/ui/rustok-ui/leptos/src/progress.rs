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
use rustok_ui::{
    Orientation, normalize_progress_max, normalize_progress_value_for_max,
    progress_classes, progress_value_percentage,
};

/// Accessible progress primitive with Radix-compatible range and state props.
///
/// `value` defaults to zero and is clamped to `0..=max`; non-finite values
/// render as zero. The SVG indicator uses the corresponding percentage while
/// ARIA exposes the configured value range. `max` defaults to `100`. Name the
/// progress bar with `aria_label` or an `aria_labelledby` reference.
#[component]
pub fn Progress(
    #[prop(optional, into)] id: String,
    #[prop(optional, into)] value: MaybeProp<f64>,
    #[prop(optional, into)] aria_label: String,
    #[prop(optional, into)] max: MaybeProp<f64>,
    #[prop(default = Orientation::Horizontal)] orientation: Orientation,
    #[prop(optional, into)] aria_labelledby: String,
    #[prop(optional, into)] aria_value_text: String,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let id = (!id.is_empty()).then_some(id);
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = progress_classes(custom);
    let value_prop = value;
    let max_prop = max;
    let max_signal =
        Signal::derive(move || normalize_progress_max(max_prop.get().unwrap_or(100.0)));
    let value_signal = Signal::derive(move || {
        normalize_progress_value_for_max(value_prop.get().unwrap_or(0.0), max_signal.get())
    });
    let percent = move || {
        progress_value_percentage(
            value_prop.get().unwrap_or(0.0),
            max_signal.get(),
        )
        .to_string()
    };
    let aria_value = move || value_signal.get().to_string();
    let data_state = move || {
        if value_signal.get() >= max_signal.get() {
            "complete"
        } else {
            "loading"
        }
    };
    let aria_label = (!aria_label.is_empty()).then_some(aria_label);
    let aria_labelledby = (!aria_labelledby.is_empty()).then_some(aria_labelledby);
    let aria_value_text = (!aria_value_text.is_empty()).then_some(aria_value_text);
    let orientation_value = orientation.as_str();

    view! {
        <div
            id=id
            role="progressbar"
            data-slot="progress"
            data-state=data_state
            data-value=move || value_signal.get().to_string()
            data-max=move || max_signal.get().to_string()
            data-orientation=orientation_value
            aria-label=aria_label
            aria-labelledby=aria_labelledby
            aria-valuemin="0"
            aria-valuemax=move || max_signal.get().to_string()
            aria-valuenow=aria_value
            aria-valuetext=aria_value_text
            aria-orientation=orientation_value
            class=full_class
        >
            <svg
                aria-hidden="true"
                class="block h-full w-full"
                preserveAspectRatio="none"
                viewBox="0 0 100 2"
            >
                <rect
                    class="fill-primary transition-all"
                    height="2"
                    width=percent
                    x="0"
                    y="0"
                />
            </svg>
        </div>
    }
}
