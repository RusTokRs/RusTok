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
use crate::i18n::t;

#[component]
pub fn SecurityStorefrontView() -> impl IntoView {
    view! {
        <div class="rustok-security-storefront-widget p-4">
            <h2 class="text-xl font-semibold">{t(None, "security-storefront-title", "Security")}</h2>
            <p class="text-muted-foreground">{t(None, "security-storefront-description", "Security Suite and Threat Protection module for RusToK")}</p>
        </div>
    }
}
