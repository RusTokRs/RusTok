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
use rustok_ui::{AvatarSize, avatar_classes, extract_initials};

#[component]
pub fn Avatar(
    #[prop(optional, into)] src: Option<String>,
    #[prop(optional, into)] alt: Option<String>,
    #[prop(optional, into)] fallback: Option<String>,
    #[prop(default = AvatarSize::Md)] size: AvatarSize,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let (container_class, fallback_class) = avatar_classes(size, custom);

    let initials = extract_initials(fallback.as_deref().unwrap_or(""));

    view! {
        <div class=container_class>
            {match src {
                Some(url) => view! {
                    <img
                        src=url
                        alt=alt.unwrap_or_default()
                        class="aspect-square h-full w-full object-cover"
                    />
                }.into_any(),
                None => view! {
                    <span class=fallback_class>
                        {initials}
                    </span>
                }.into_any(),
            }}
        </div>
    }
}
