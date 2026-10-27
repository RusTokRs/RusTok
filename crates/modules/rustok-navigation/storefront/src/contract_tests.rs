/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use super::*;

#[test]
fn test_storefront_menu_location_variants() {
    assert_eq!(StorefrontMenuLocation::Header, StorefrontMenuLocation::Header);
    assert_ne!(StorefrontMenuLocation::Header, StorefrontMenuLocation::Footer);
    assert_ne!(StorefrontMenuLocation::Sidebar, StorefrontMenuLocation::Mobile);

    let locations = [
        StorefrontMenuLocation::Header,
        StorefrontMenuLocation::Footer,
        StorefrontMenuLocation::Sidebar,
        StorefrontMenuLocation::Mobile,
    ];
    assert_eq!(locations.len(), 4);
}

#[test]
fn test_storefront_navigation_snapshot() {
    let header_menu = StorefrontMenu {
        id: "menu-1".to_string(),
        effective_locale: "en".to_string(),
        name: "Main Header".to_string(),
        location: StorefrontMenuLocation::Header,
        items: vec![StorefrontMenuItem {
            id: "item-1".to_string(),
            title: "Home".to_string(),
            url: "/".to_string(),
            icon: None,
            children: vec![],
        }],
    };

    let snapshot = StorefrontNavigationSnapshot {
        header: Some(header_menu.clone()),
        footer: None,
    };

    assert_eq!(snapshot.menu(StorefrontMenuLocation::Header), Some(&header_menu));
    assert_eq!(snapshot.menu(StorefrontMenuLocation::Footer), None);
    assert_eq!(snapshot.menu(StorefrontMenuLocation::Sidebar), None);
    assert_eq!(snapshot.menu(StorefrontMenuLocation::Mobile), None);
}

#[test]
fn test_storefront_menu_item_tree() {
    let child = StorefrontMenuItem {
        id: "child-1".to_string(),
        title: "Shoes".to_string(),
        url: "/catalog/shoes".to_string(),
        icon: Some("shoe-icon".to_string()),
        children: vec![],
    };

    let parent = StorefrontMenuItem {
        id: "parent-1".to_string(),
        title: "Catalog".to_string(),
        url: "/catalog".to_string(),
        icon: None,
        children: vec![child.clone()],
    };

    assert_eq!(parent.children.len(), 1);
    assert_eq!(parent.children[0].title, "Shoes");
    assert_eq!(parent.children[0].icon.as_deref(), Some("shoe-icon"));
}

#[test]
fn test_crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Public surface",
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}
