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
use rustok_ui::{BadgeVariant, Size, badge_classes};

#[component]
pub fn Badge(
    #[prop(default = BadgeVariant::Default)] variant: BadgeVariant,
    #[prop(default = Size::Md)] size: Size,
    #[prop(default = false)] dismissible: bool,
    #[prop(optional)] on_dismiss: Option<Callback<()>>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = badge_classes(variant, size, custom);

    view! {
        <span class=full_class>
            {children()}
            {move || dismissible.then(|| {
                view! {
                    <button
                        type="button"
                        class="ml-0.5 rounded-full opacity-70 hover:opacity-100 focus:outline-none"
                        aria-label="Dismiss"
                        on:click=move |_| {
                            if let Some(cb) = on_dismiss {
                                cb.run(());
                            }
                        }
                    >
                        "×"
                    </button>
                }
            })}
        </span>
    }
}
