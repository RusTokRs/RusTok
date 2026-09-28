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
use rustok_ui::{ButtonVariant, Size, button_classes};

use crate::spinner::Spinner;

#[component]
pub fn Button(
    #[prop(default = ButtonVariant::Default)] variant: ButtonVariant,
    #[prop(default = Size::Md)] size: Size,
    #[prop(default = false)] disabled: bool,
    #[prop(default = false)] loading: bool,
    #[prop(optional, into)] class: String,
    #[prop(optional)] on_click: Option<Box<dyn Fn() + 'static>>,
    #[prop(default = "button")] r#type: &'static str,
    children: Children,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = button_classes(variant, size, disabled, loading, custom);
    let is_disabled = disabled || loading;

    view! {
        <button
            type=r#type
            class=full_class
            disabled=is_disabled
            on:click=move |_| {
                if let Some(ref handler) = on_click {
                    handler();
                }
            }
        >
            {move || {
                if loading {
                    Some(view! { <Spinner size=Size::Sm class="mr-2" /> })
                } else {
                    None
                }
            }}
            {children()}
        </button>
    }
}
