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
use rustok_ui::merge_classes;

#[component]
pub fn SuccessMessage(
    #[prop(into)] message: String,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let base_class = "rounded-lg border border-emerald-500/50 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-600 dark:text-emerald-400";
    let full_class = merge_classes(&[base_class, class.as_str()]);
    view! {
        <div
            role="status"
            data-slot="success-message"
            class=full_class
        >
            {message}
        </div>
    }
}
