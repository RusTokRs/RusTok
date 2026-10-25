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
    let ctx = BrandAdminTransportContext::native();
    assert_eq!(ctx.profile, BrandAdminTransportProfile::Native);
    assert_eq!(ctx.access_token, None);
    assert_eq!(ctx.tenant_slug, None);
}

#[test]
fn test_transport_context_graphql() {
    let ctx = BrandAdminTransportContext::graphql(
        Some("brand_token".to_string()),
        Some("tenant_slug".to_string()),
    );
    assert_eq!(ctx.profile, BrandAdminTransportProfile::Graphql);
    assert_eq!(ctx.access_token.as_deref(), Some("brand_token"));
    assert_eq!(ctx.tenant_slug.as_deref(), Some("tenant_slug"));
}

#[test]
fn test_selected_transport_profile() {
    assert_eq!(
        selected_transport_profile(Some("graphql")),
        BrandAdminTransportProfile::Graphql
    );
    assert_eq!(
        selected_transport_profile(Some("native")),
        BrandAdminTransportProfile::Native
    );
    assert_eq!(
        selected_transport_profile(None),
        BrandAdminTransportProfile::Native
    );
}

#[test]
fn test_validate_brand_slug() {
    assert!(validate_brand_slug("acme-corp").is_ok());
    assert!(validate_brand_slug("brand_123").is_ok());
    assert!(validate_brand_slug("").is_err());
    assert!(validate_brand_slug("brand with spaces").is_err());
    assert!(validate_brand_slug(&"a".repeat(101)).is_err());
}

#[test]
fn test_validate_brand_name() {
    assert!(validate_brand_name("Acme Corporation").is_ok());
    assert!(validate_brand_name("").is_err());
    assert!(validate_brand_name("   ").is_err());
    assert!(validate_brand_name(&"a".repeat(256)).is_err());
}

#[test]
fn test_brand_shell_resolution() {
    let shell_en = build_brand_admin_shell(Some("en"), BrandAdminTransportProfile::Native);
    assert_eq!(shell_en.title, "Brands");
    assert_eq!(shell_en.transport_profile, "native");

    let shell_ru = build_brand_admin_shell(Some("ru"), BrandAdminTransportProfile::Graphql);
    assert_eq!(shell_ru.title, "Бренды");
    assert_eq!(shell_ru.transport_profile, "graphql");
}

#[test]
fn test_fallback_policy() {
    assert_eq!(BRAND_TRANSPORT_FALLBACK_POLICY, "never falls back");
}
