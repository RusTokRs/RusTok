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

    let src = src.and_then(|s| (!s.trim().is_empty()).then_some(s));
    let initials = extract_initials(fallback.as_deref().unwrap_or(""));
    let (img_error, set_img_error) = signal(false);

    let src_check = src.clone();
    let has_image = move || src_check.is_some() && !img_error.get();

    view! {
        <div data-slot="avatar" class=container_class>
            {move || {
                if has_image() {
                    let url = src.clone().unwrap_or_default();
                    view! {
                        <img
                            data-slot="avatar-image"
                            src=url
                            alt=alt.clone().unwrap_or_default()
                            class="aspect-square h-full w-full object-cover"
                            on:error=move |_| set_img_error.set(true)
                        />
                    }.into_any()
                } else {
                    view! {
                        <span data-slot="avatar-fallback" class=fallback_class.clone()>
                            {initials.clone()}
                        </span>
                    }.into_any()
                }
            }}
        </div>
    }
}
