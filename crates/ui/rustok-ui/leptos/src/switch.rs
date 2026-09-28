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
use rustok_ui::{SwitchSize, switch_classes};

#[component]
pub fn Switch(
    #[prop(optional)] checked: Option<ReadSignal<bool>>,
    #[prop(optional)] set_checked: Option<WriteSignal<bool>>,
    #[prop(default = false)] disabled: bool,
    #[prop(default = SwitchSize::Md)] size: SwitchSize,
    #[prop(optional, into)] class: String,
    #[prop(optional, into)] id: String,
) -> impl IntoView {
    let custom = (!class.is_empty()).then_some(class.as_str());
    let is_checked = move || checked.map(|c| c.get()).unwrap_or(false);

    let (track_classes, thumb_classes) = {
        let (track_off, thumb_off) = switch_classes(false, size, disabled, custom);
        let (track_on, thumb_on) = switch_classes(true, size, disabled, custom);
        (
            move || if is_checked() { track_on.clone() } else { track_off.clone() },
            move || if is_checked() { thumb_on.clone() } else { thumb_off.clone() },
        )
    };

    view! {
        <button
            id=id
            type="button"
            role="switch"
            aria-checked=move || is_checked().to_string()
            disabled=disabled
            class=track_classes
            on:click=move |_| {
                if !disabled
                    && let Some(set) = set_checked
                {
                    let current = checked.map(|c| c.get()).unwrap_or(false);
                    set.set(!current);
                }
            }
        >
            <span class=thumb_classes />
        </button>
    }
}
