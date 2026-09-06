/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::sync::OnceLock;

use leptos::prelude::use_context;
use rustok_ui_core::UiRouteContext;
use rustok_ui_i18n::{UiMessageCatalog, build_ui_message_catalog, resolve_ui_message_or_fallback};

pub struct LeptosUiMessages {
    default_locale: &'static str,
    bundles: &'static [(&'static str, &'static str)],
    catalog: OnceLock<UiMessageCatalog>,
}

impl LeptosUiMessages {
    pub const fn new(
        default_locale: &'static str,
        bundles: &'static [(&'static str, &'static str)],
    ) -> Self {
        Self {
            default_locale,
            bundles,
            catalog: OnceLock::new(),
        }
    }

    pub fn catalog(&self) -> &UiMessageCatalog {
        self.catalog
            .get_or_init(|| build_ui_message_catalog(self.bundles))
    }

    pub fn t_for_locale(&self, locale: Option<&str>, key: &str, fallback: &str) -> String {
        resolve_ui_message_or_fallback(self.catalog(), locale, self.default_locale, key, fallback)
    }

    pub fn t_from_context(&self, key: &str, fallback: &str) -> String {
        let locale = use_context::<UiRouteContext>().and_then(|context| context.locale);
        self.t_for_locale(locale.as_deref(), key, fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::LeptosUiMessages;

    static MESSAGES: LeptosUiMessages = LeptosUiMessages::new(
        "en",
        &[
            ("en", r#"{ "title": "Dashboard" }"#),
            ("ru", r#"{ "title": "Dashboard RU" }"#),
        ],
    );

    #[test]
    fn resolves_explicit_locale_through_core_catalog() {
        assert_eq!(
            MESSAGES.t_for_locale(Some("ru-RU"), "title", "Fallback"),
            "Dashboard RU"
        );
        assert_eq!(
            MESSAGES.t_for_locale(Some("fr"), "missing", "Fallback"),
            "Fallback"
        );
    }
}
