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
use rustok_ui::{SwitchSize, switch_classes};

#[component]
pub fn Switch(
    #[props(default = false)] checked: bool,
    #[props(default = false)] disabled: bool,
    #[props(default)] size: SwitchSize,
    #[props(default)] ontoggle: Option<EventHandler<bool>>,
    #[props(default)] class: Option<String>,
    #[props(default)] id: Option<String>,
) -> Element {
    let custom = class.as_deref();
    let (track_class, thumb_class) = switch_classes(checked, size, disabled, custom);

    rsx! {
        button {
            id: id.as_deref(),
            r#type: "button",
            role: "switch",
            "aria-checked": checked.to_string(),
            disabled: disabled,
            class: "{track_class}",
            onclick: move |_| {
                if !disabled && let Some(ref cb) = ontoggle {
                    cb.call(!checked);
                }
            },
            span { class: "{thumb_class}" }
        }
    }
}
