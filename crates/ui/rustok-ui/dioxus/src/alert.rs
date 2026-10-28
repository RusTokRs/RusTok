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
use rustok_ui::{AlertVariant, alert_classes};

#[component]
pub fn Alert(
    #[props(default)] variant: AlertVariant,
    #[props(default)] title: Option<String>,
    #[props(default)] class: Option<String>,
    children: Element,
) -> Element {
    let custom = class.as_deref();
    let full_class = alert_classes(variant, custom);

    rsx! {
        div {
            role: "alert",
            class: "{full_class}",
            if let Some(t) = title {
                p { class: "mb-1 font-medium leading-none tracking-tight", "{t}" }
            }
            div { class: "[&_p]:leading-relaxed",
                {children}
            }
        }
    }
}
