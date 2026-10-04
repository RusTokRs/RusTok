/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use dioxus::prelude::*;
use rustok_ui::{
    Orientation, normalize_progress_max, normalize_progress_value_for_max,
    progress_classes, progress_value_percentage,
};

/// Accessible progress primitive with a configurable ARIA range.
///
/// `value` is clamped to `0..=max`, and non-finite values render as zero. The
/// SVG indicator uses the corresponding percentage while ARIA exposes the
/// original value range. Pass an accessible name through `aria_label` or
/// `aria_labelledby`.
#[component]
pub fn Progress(
    #[props(default)] id: Option<String>,
    #[props(default = 0.0)] value: f64,
    #[props(default)] aria_label: Option<String>,
    #[props(default = 100.0)] max: f64,
    #[props(default = Orientation::Horizontal)] orientation: Orientation,
    #[props(default)] aria_labelledby: Option<String>,
    #[props(default)] aria_value_text: Option<String>,
    #[props(default)] class: Option<String>,
) -> Element {
    let max = normalize_progress_max(max);
    let value = normalize_progress_value_for_max(value, max);
    let value_text = value.to_string();
    let width = progress_value_percentage(value, max).to_string();
    let data_state = if value >= max {
        "complete"
    } else {
        "loading"
    };
    let orientation = orientation.as_str();
    let full_class = progress_classes(class.as_deref());

    rsx! {
        div {
            id: id.as_deref(),
            role: "progressbar",
            "data-slot": "progress",
            "data-state": data_state,
            "data-value": "{value_text}",
            "data-max": "{max}",
            "data-orientation": orientation,
            "aria-label": aria_label.as_deref(),
            "aria-labelledby": aria_labelledby.as_deref(),
            "aria-valuemin": "0",
            "aria-valuemax": "{max}",
            "aria-valuenow": "{value_text}",
            "aria-valuetext": aria_value_text.as_deref(),
            "aria-orientation": orientation,
            class: "{full_class}",
            svg {
                "aria-hidden": "true",
                class: "block h-full w-full",
                preserve_aspect_ratio: "none",
                view_box: "0 0 100 2",
                rect {
                    class: "fill-primary transition-all",
                    height: "2",
                    width: "{width}",
                    x: "0",
                    y: "0",
                }
            }
        }
    }
}
