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
use rustok_ui::{AvatarSize, avatar_classes, extract_initials};

#[component]
pub fn Avatar(
    #[props(default)] src: Option<String>,
    #[props(default)] alt: Option<String>,
    #[props(default)] fallback: Option<String>,
    #[props(default)] size: AvatarSize,
    #[props(default)] class: Option<String>,
) -> Element {
    let custom = class.as_deref();
    let (container_class, fallback_class) = avatar_classes(size, custom);
    let initials = extract_initials(fallback.as_deref().unwrap_or(""));

    rsx! {
        div { class: "{container_class}",
            match src {
                Some(url) => rsx! {
                    img {
                        src: "{url}",
                        alt: alt.as_deref().unwrap_or(""),
                        class: "aspect-square h-full w-full object-cover",
                    }
                },
                None => rsx! {
                    span { class: "{fallback_class}",
                        "{initials}"
                    }
                }
            }
        }
    }
}
