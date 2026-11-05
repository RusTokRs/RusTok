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
use rustok_ui::{BadgeVariant, Size, badge_classes};

#[component]
pub fn Badge(
    #[props(default)] variant: BadgeVariant,
    #[props(default)] size: Size,
    #[props(default = false)] dismissible: bool,
    #[props(default)] on_dismiss: Option<EventHandler<()>>,
    #[props(default)] class: Option<String>,
    children: Element,
) -> Element {
    let custom = class.as_deref();
    let full_class = badge_classes(variant, size, custom);

    rsx! {
        span {
            class: "{full_class}",
            {children}
            if dismissible {
                button {
                    r#type: "button",
                    class: "ml-0.5 rounded-full opacity-70 hover:opacity-100 focus:outline-none",
                    "aria-label": "Dismiss",
                    onclick: move |_| {
                        if let Some(ref cb) = on_dismiss {
                            cb.call(());
                        }
                    },
                    "×"
                }
            }
        }
    }
}
