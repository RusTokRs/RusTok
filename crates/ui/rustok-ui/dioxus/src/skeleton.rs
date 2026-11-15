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
use rustok_ui::{SkeletonVariant, skeleton_classes};

#[component]
pub fn Skeleton(
    #[props(default)] variant: SkeletonVariant,
    #[props(default)] id: Option<String>,
    #[props(default)] class: Option<String>,
) -> Element {
    let id = id.as_deref().filter(|s| !s.trim().is_empty());
    let custom = class.as_deref().filter(|s| !s.trim().is_empty());
    let full_class = skeleton_classes(variant, custom);

    rsx! {
        div {
            id: id,
            "data-slot": "skeleton",
            class: "{full_class}",
            "aria-hidden": "true",
        }
    }
}
