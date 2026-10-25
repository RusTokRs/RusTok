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
fn test_transport_context_native() {
    let ctx = ProductRelationsTransportContext::native();
    assert_eq!(ctx.profile, ProductRelationsTransportProfile::Native);
    assert_eq!(ctx.access_token, None);
    assert_eq!(ctx.tenant_slug, None);
}

#[test]
fn test_transport_context_graphql() {
    let ctx = ProductRelationsTransportContext::graphql(
        Some("token_123".to_string()),
        Some("tenant_abc".to_string()),
    );
    assert_eq!(ctx.profile, ProductRelationsTransportProfile::Graphql);
    assert_eq!(ctx.access_token.as_deref(), Some("token_123"));
    assert_eq!(ctx.tenant_slug.as_deref(), Some("tenant_abc"));
}

#[test]
fn test_selected_transport_profile() {
    assert_eq!(
        selected_transport_profile(Some("graphql")),
        ProductRelationsTransportProfile::Graphql
    );
    assert_eq!(
        selected_transport_profile(Some("GRAPHQL")),
        ProductRelationsTransportProfile::Graphql
    );
    assert_eq!(
        selected_transport_profile(Some("native")),
        ProductRelationsTransportProfile::Native
    );
    assert_eq!(
        selected_transport_profile(None),
        ProductRelationsTransportProfile::Native
    );
}

#[test]
fn test_target_product_id_validation() {
    assert!(validate_target_product_id("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").is_ok());
    assert!(validate_target_product_id("").is_err());
    assert!(validate_target_product_id("   ").is_err());
    assert!(validate_target_product_id("invalid-uuid").is_err());
}

#[test]
fn test_valid_relation_types() {
    assert!(is_valid_relation_type("cross_sell"));
    assert!(is_valid_relation_type("up_sell"));
    assert!(is_valid_relation_type("related"));
    assert!(is_valid_relation_type("accessory"));
    assert!(is_valid_relation_type("alternative"));
    assert!(!is_valid_relation_type("unknown_relation"));
}

#[test]
fn test_panel_copy_resolution() {
    let copy_en = build_product_relations_panel_copy(Some("en"));
    assert_eq!(copy_en.title, "Product Relations");
    assert_eq!(copy_en.tab_cross_sell, "Cross-sell");
    assert_eq!(copy_en.tab_up_sell, "Up-sell");

    let copy_ru = build_product_relations_panel_copy(Some("ru"));
    assert_eq!(copy_ru.title, "Связанные товары");
    assert_eq!(copy_ru.tab_cross_sell, "Сопутствующие");
}

#[test]
fn test_fallback_policy() {
    assert_eq!(PRODUCT_RELATIONS_FALLBACK_POLICY, "never falls back");
}
