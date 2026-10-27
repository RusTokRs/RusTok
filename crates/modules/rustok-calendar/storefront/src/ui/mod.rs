/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use crate::i18n::t;
use leptos::prelude::*;

#[component]
pub fn CalendarStorefrontView() -> impl IntoView {
    view! {
        <div class="rustok-calendar-storefront-widget p-4">
            <h2 class="text-xl font-semibold">{t(None, "calendar-storefront-title", "Calendar & Appointment Scheduling")}</h2>
            <p class="text-muted-foreground">{t(None, "calendar-storefront-description", "Calendar & Appointment Scheduling capability module for RusToK")}</p>
        </div>
    }
}
