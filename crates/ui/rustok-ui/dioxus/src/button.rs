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
use rustok_ui::{ButtonVariant, Size, button_classes};

use crate::spinner::Spinner;

#[component]
pub fn Button(
    #[props(default)] variant: ButtonVariant,
    #[props(default)] size: Size,
    #[props(default = false)] disabled: bool,
    #[props(default = false)] loading: bool,
    #[props(default)] class: Option<String>,
    #[props(default)] onclick: Option<EventHandler<MouseEvent>>,
    #[props(default = "button")] r#type: &'static str,
    children: Element,
) -> Element {
    let custom = class.as_deref();
    let full_class = button_classes(variant, size, disabled, loading, custom);
    let is_disabled = disabled || loading;
    let type_attr = r#type;

    rsx! {
        button {
            r#type: "{type_attr}",
            class: "{full_class}",
            disabled: is_disabled,
            onclick: move |evt| {
                if let Some(ref handler) = onclick {
                    handler.call(evt);
                }
            },
            if loading {
                Spinner { size: Size::Sm, class: "mr-2".to_string() }
            }
            {children}
        }
    }
}
