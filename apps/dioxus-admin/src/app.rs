/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use crate::shell::DioxusHostContext;
use rustok_ui_core::UiRouteContext;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DioxusNavigationItem {
    pub slug: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub route: &'static str,
}

pub const STANDARD_NAVIGATION: &[DioxusNavigationItem] = &[
    DioxusNavigationItem { slug: "dashboard", label: "Dashboard", icon: "layout-dashboard", route: "/dashboard" },
    DioxusNavigationItem { slug: "products", label: "Catalog", icon: "package", route: "/modules/product" },
    DioxusNavigationItem { slug: "pages", label: "Pages", icon: "file-text", route: "/modules/pages" },
    DioxusNavigationItem { slug: "orders", label: "Orders", icon: "shopping-bag", route: "/modules/order" },
    DioxusNavigationItem { slug: "settings", label: "Settings", icon: "settings", route: "/settings" },
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DioxusAdminApp {
    pub locale: String,
    pub tenant: String,
    pub active_route: String,
}

impl DioxusAdminApp {
    pub fn new(locale: impl Into<String>, tenant: impl Into<String>) -> Self {
        Self {
            locale: locale.into(),
            tenant: tenant.into(),
            active_route: "/dashboard".to_string(),
        }
    }

    pub fn host_context(&self) -> DioxusHostContext {
        DioxusHostContext::new(&self.locale, &self.tenant, &self.active_route)
    }

    pub fn ui_route_context(&self) -> UiRouteContext {
        self.host_context().to_ui_route_context()
    }

    pub fn navigation(&self) -> &'static [DioxusNavigationItem] {
        STANDARD_NAVIGATION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dioxus_admin_app_initialization() {
        let app = DioxusAdminApp::new("en", "main-tenant");
        assert_eq!(app.navigation().len(), 5);
        let ctx = app.ui_route_context();
        assert_eq!(ctx.locale.as_deref(), Some("en"));
    }
}
