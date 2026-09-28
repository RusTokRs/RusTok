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
use rustok_ui::{Size, spinner_classes};

#[component]
pub fn Spinner(
    #[props(default)] size: Size,
    #[props(default)] class: Option<String>,
) -> Element {
    let custom = class.as_deref();
    let full_class = spinner_classes(size, custom);

    rsx! {
        span {
            role: "status",
            "aria-label": "Loading",
            class: "{full_class}",
        }
    }
}
