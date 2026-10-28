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
use rustok_ui::{AlertVariant, alert_classes};

#[component]
pub fn Alert(
    #[prop(default = AlertVariant::Default)] variant: AlertVariant,
    #[prop(optional, into)] title: Option<String>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = alert_classes(variant, custom);

    view! {
        <div
            role="alert"
            class=full_class
        >
            {title.map(|t| view! {
                <p class="mb-1 font-medium leading-none tracking-tight">{t}</p>
            })}
            <div class="[&_p]:leading-relaxed">
                {children()}
            </div>
        </div>
    }
}
