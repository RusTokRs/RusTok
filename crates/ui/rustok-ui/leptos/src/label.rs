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

/// Field label with `data-slot="label"` and accessible target reference.
///
/// `r#for` targets an element id; `required` appends the destructive
/// asterisk, and `disabled` switches the `peer-disabled:` styling to the
/// hard-disabled one for labels that are not peers of their control.
#[component]
pub fn Label(
    #[prop(default = false)] required: bool,
    #[prop(default = false)] disabled: bool,
    #[prop(optional, into)] r#for: Option<String>,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let full_class = label_classes(disabled, custom);
    let for_attr = r#for.and_then(|f| (!f.is_empty()).then_some(f));

    view! {
        <label
            data-slot="label"
            for=for_attr
            class=full_class
        >
            {children()}
            {move || required.then(|| view! { <span class="text-destructive ml-1">"*"</span> })}
        </label>
    }
}
