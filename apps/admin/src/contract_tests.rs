/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use leptos::prelude::IntoAny;

use crate::app::modules::{
    AdminChildPageRegistration, AdminPageRegistration, AdminSlot,
};
use crate::app::providers::locale::Locale;
use crate::app::security::validate_admin_security_profile;
use crate::i18n::{t, validate};

#[test]
fn test_locale_normalization_contract() {
    assert_eq!(Locale::from_effective_locale(None), Locale::En);
    assert_eq!(Locale::from_effective_locale(Some("en")), Locale::En);
    assert_eq!(Locale::from_effective_locale(Some("en-US")), Locale::En);
    assert_eq!(Locale::from_effective_locale(Some("ru")), Locale::Ru);
    assert_eq!(Locale::from_effective_locale(Some("ru-RU")), Locale::Ru);
    assert_eq!(Locale::from_effective_locale(Some("unknown")), Locale::En);

    assert_eq!(Locale::En.as_str(), "en");
    assert_eq!(Locale::Ru.as_str(), "ru");
}

#[test]
fn test_host_i18n_catalog_contract() {
    assert!(
        validate().is_ok(),
        "Host i18n catalog must validate successfully: {:?}",
        validate().err()
    );

    let en_dash = t(Some("en"), "app.nav.dashboard", "fallback");
    let ru_dash = t(Some("ru"), "app.nav.dashboard", "fallback");
    assert_eq!(en_dash, "Dashboard");
    assert_eq!(ru_dash, "Дашборд");
    assert_ne!(en_dash, ru_dash);

    let en_modules = t(Some("en"), "modules.build.progress", "fallback");
    let ru_modules = t(Some("ru"), "modules.build.progress", "fallback");
    assert_eq!(en_modules, "Platform build progress");
    assert_eq!(ru_modules, "Прогресс сборки платформы");
}

#[test]
fn test_admin_page_registration_contract() {
    static TEST_CHILDREN: &[AdminChildPageRegistration] = &[AdminChildPageRegistration {
        subpath: "settings",
        title: "Settings",
        nav_label: "Settings",
    }];

    let page = AdminPageRegistration {
        module_slug: "test-module",
        route_segment: "test",
        title: "Test Module",
        child_pages: TEST_CHILDREN,
        render: || ().into_any(),
    };

    assert_eq!(page.module_slug, "test-module");
    assert_eq!(page.route_segment, "test");
    assert_eq!(page.title, "Test Module");
    assert_eq!(page.child_pages.len(), 1);
    assert_eq!(page.child_pages[0].subpath, "settings");
    assert_eq!(page.child_pages[0].title, "Settings");
    assert_eq!(page.child_pages[0].nav_label, "Settings");
    assert_eq!(
        page.child_page_for_subpath("settings")
            .map(|c| c.title),
        Some("Settings")
    );
    assert!(page.child_page_for_subpath("nonexistent").is_none());
}

#[test]
fn test_admin_slot_variants() {
    assert_eq!(AdminSlot::DashboardSection, AdminSlot::DashboardSection);
    assert_eq!(AdminSlot::NavItem, AdminSlot::NavItem);
    assert_ne!(AdminSlot::DashboardSection, AdminSlot::NavItem);
}

#[test]
fn test_security_profile_validation() {
    // In dev / test environment where RUSTOK_ENV is not "production", validation succeeds
    assert!(validate_admin_security_profile().is_ok());
}
