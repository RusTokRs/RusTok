/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_admin_dioxus::DioxusAdminApp;

fn main() {
    println!("Starting RusToK Dioxus Admin Host Shell (scaffolding)...");
    let app = DioxusAdminApp::new("en", "default-tenant");
    println!("Initialized Dioxus Admin Shell: locale={}, tenant={}", app.locale, app.tenant);
}
