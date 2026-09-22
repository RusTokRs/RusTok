/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_ui_core::UiRouteContext;
use serde::{Deserialize, Serialize};

/// Host shell context providing route, tenant, and effective locale state
/// to framework adapters without framework coupling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DioxusHostContext {
    pub locale: String,
    pub tenant: String,
    pub active_route: String,
}

impl DioxusHostContext {
    pub fn new(locale: impl Into<String>, tenant: impl Into<String>, route: impl Into<String>) -> Self {
        Self {
            locale: locale.into(),
            tenant: tenant.into(),
            active_route: route.into(),
        }
    }

    /// Converts this host context to the canonical framework-agnostic [`UiRouteContext`].
    pub fn to_ui_route_context(&self) -> UiRouteContext {
        UiRouteContext {
            locale: Some(self.locale.clone()),
            route_segment: Some(self.active_route.trim_matches('/').to_string()),
            subpath: None,
            query: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dioxus_host_context_to_ui_route_context() {
        let host = DioxusHostContext::new("ru", "tenant-alpha", "pages");
        let ui_context = host.to_ui_route_context();
        assert_eq!(ui_context.locale.as_deref(), Some("ru"));
        assert_eq!(ui_context.module_route_base("pages"), "/ru/modules/pages");
    }
}
