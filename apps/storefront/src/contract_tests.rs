/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::collections::{HashMap, HashSet};

use leptos::prelude::IntoAny;

use crate::modules::{
    components_for_slot, page_for_route_segment, register_component, register_page,
    StorefrontComponentRegistration, StorefrontPageLookup, StorefrontPageRegistration,
    StorefrontSlot,
};
use crate::shared::context::canonical_route::{
    build_module_route, build_redirect_location, ResolvedCanonicalRoute,
};
use crate::shared::local::{featured_products, locale_strings};

#[test]
fn test_storefront_i18n_catalog_contract() {
    let en = locale_strings("en");
    let ru = locale_strings("ru");

    assert_eq!(en.nav_home, "Home");
    assert_eq!(ru.nav_home, "Главная");
    assert_ne!(en.hero_title, ru.hero_title);
    assert_eq!(en.cta_primary, "Browse catalog");
    assert_eq!(ru.cta_primary, "Открыть каталог");

    let products_en = featured_products("en");
    let products_ru = featured_products("ru");
    assert!(!products_en.is_empty());
    assert_eq!(products_en.len(), products_ru.len());
    assert_ne!(products_en[0].title, products_ru[0].title);
}

#[test]
fn test_storefront_slots_and_component_registry() {
    assert_eq!(
        StorefrontSlot::HeaderNavigation,
        StorefrontSlot::HeaderNavigation
    );
    assert_ne!(
        StorefrontSlot::HeaderNavigation,
        StorefrontSlot::FooterNavigation
    );

    register_component(StorefrontComponentRegistration {
        id: "test-contract-component",
        module_slug: Some("test_slug"),
        slot: StorefrontSlot::HeaderActions,
        order: 10,
        render: || ().into_any(),
    });

    let enabled = HashSet::new();
    let components = components_for_slot(StorefrontSlot::HeaderActions, Some(&enabled));
    assert!(
        components.iter().all(|c| c.id != "test-contract-component"),
        "Disabled module component must be filtered out"
    );

    let mut enabled_mut = HashSet::new();
    enabled_mut.insert("test_slug".to_string());
    let components_enabled = components_for_slot(StorefrontSlot::HeaderActions, Some(&enabled_mut));
    assert!(
        components_enabled
            .iter()
            .any(|c| c.id == "test-contract-component"),
        "Enabled module component must be returned"
    );
}

#[test]
fn test_storefront_page_registry_lookup_contract() {
    register_page(StorefrontPageRegistration {
        module_slug: "contract-test-module",
        route_segment: "contract-test-route",
        title: "Contract Test Page",
        full_width: false,
        render: || ().into_any(),
    });

    let enabled: HashSet<String> = HashSet::new();
    match page_for_route_segment("contract-test-route", Some(&enabled)) {
        StorefrontPageLookup::Disabled => {}
        StorefrontPageLookup::Found(_) => panic!("Expected Disabled, got Found"),
        StorefrontPageLookup::Missing => panic!("Expected Disabled, got Missing"),
    }

    let mut enabled_with_module = HashSet::new();
    enabled_with_module.insert("contract-test-module".to_string());
    match page_for_route_segment("contract-test-route", Some(&enabled_with_module)) {
        StorefrontPageLookup::Found(page) => {
            assert_eq!(page.module_slug, "contract-test-module");
            assert_eq!(page.title, "Contract Test Page");
        }
        _ => panic!("Expected StorefrontPageLookup::Found"),
    }

    match page_for_route_segment("nonexistent-segment-xyz", Some(&enabled_with_module)) {
        StorefrontPageLookup::Missing => {}
        _ => panic!("Expected StorefrontPageLookup::Missing"),
    }
}

#[test]
fn test_canonical_route_building_contract() {
    let query_params = HashMap::from([
        ("lang".to_string(), "ru".to_string()),
        ("filter".to_string(), "active".to_string()),
        ("page".to_string(), "2".to_string()),
    ]);

    let route = build_module_route("catalog", &query_params);
    assert_eq!(route, "/modules/catalog?filter=active&page=2");

    let resolved = ResolvedCanonicalRoute {
        target_kind: "product".to_string(),
        target_id: "prod-1".to_string(),
        locale: "ru".to_string(),
        matched_url: "/modules/catalog?id=prod-1".to_string(),
        canonical_url: "/modules/products?slug=flagship".to_string(),
        redirect_required: true,
    };

    let redirect_localized = build_redirect_location(&resolved, Some("ru"), &query_params);
    assert_eq!(redirect_localized, "/ru/modules/products?slug=flagship");
}
