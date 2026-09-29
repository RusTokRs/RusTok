/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use leptos::children::Children;
use leptos::prelude::*;
use rustok_ui::label_classes;

/// Field label.
///
/// `r#for` targets a static element id; `required` appends the destructive
/// asterisk, and `disabled` switches the `peer-disabled:` styling to the
/// hard-disabled one for labels that are not peers of their control.
#[component]
pub fn Label(
    #[prop(default = false)] required: bool,
    #[prop(default = false)] disabled: bool,
    #[prop(optional)] r#for: Option<&'static str>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = label_classes(disabled, custom);

    view! {
        <label
            for=r#for
            class=full_class
        >
            {children()}
            {move || required.then(|| view! { <span class="text-destructive ml-1">"*"</span> })}
        </label>
    }
}
