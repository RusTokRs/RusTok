const CATEGORY_ROUTE: &str = include_str!("../src/services/category_route.rs");
const SERVICES_MOD: &str = include_str!("../src/services/mod.rs");
const ERROR: &str = include_str!("../src/error.rs");
const INITIAL_MIGRATION: &str =
    include_str!("../src/migrations/m20260328_000001_create_forum_tables.rs");
const SLUG_LOCALE_DECISION: &str =
    include_str!("../../../DECISIONS/2026-03-29-forum-slug-locale-contract.md");

#[test]
fn owner_uses_locale_aware_category_slug_and_existing_unique_route_key() {
    for marker in [
        "pub struct ForumCategoryRouteService",
        "pub async fn canonical_descriptor(",
        "pub async fn resolve(",
        "format!(\"/{locale}/forum/c/{slug}\")",
        "TaxonomyOwnerCategoryReader",
        "MAX_FORUM_CATEGORY_ROUTE_CANDIDATES: u64 = 64",
        "ForumCategoryRouteDisposition::Canonical",
        "ForumCategoryRouteDisposition::Redirect",
    ] {
        assert!(
            CATEGORY_ROUTE.contains(marker),
            "missing category route owner marker {marker}"
        );
    }

    for marker in [
        "idx_forum_category_translations_tenant_locale_slug",
        ".col(ForumCategoryTranslations::TenantId)",
        ".col(ForumCategoryTranslations::Locale)",
        ".col(ForumCategoryTranslations::Slug)",
        ".unique()",
    ] {
        assert!(
            INITIAL_MIGRATION.contains(marker),
            "missing existing category route persistence marker {marker}"
        );
    }

    for marker in [
        "Category slug is a locale-aware translation field",
        "locale fallback contract",
    ] {
        assert!(
            SLUG_LOCALE_DECISION.contains(marker),
            "missing accepted slug/locale decision marker {marker}"
        );
    }
}

#[test]
fn resolver_is_bounded_lifecycle_safe_and_fail_closed_on_ambiguity() {
    for marker in [
        "resolve_term_route_for_module",
        "forum_category_lifecycle::Entity::find()",
        "ensure_active_category",
        "Err(ForumError::CategoryRouteNotFound)",
        "Err(ForumError::CategoryRouteResolutionConflict)",
    ] {
        assert!(
            CATEGORY_ROUTE.contains(marker),
            "missing bounded category route marker {marker}"
        );
    }

    for marker in [
        "CategoryRouteNotFound",
        "FORUM_CATEGORY_ROUTE_NOT_FOUND",
        "CategoryRouteResolutionConflict",
        "FORUM_CATEGORY_ROUTE_RESOLUTION_CONFLICT",
    ] {
        assert!(
            ERROR.contains(marker),
            "missing typed error marker {marker}"
        );
    }
}

#[test]
fn owner_is_exported_without_transport_storage_or_visibility_policy() {
    assert_eq!(SERVICES_MOD.matches("mod category_route;").count(), 1);
    for marker in [
        "ForumCategoryRouteDescriptor",
        "ForumCategoryRouteDisposition",
        "ForumCategoryRouteResolution",
        "ForumCategoryRouteService",
    ] {
        assert!(
            SERVICES_MOD.contains(marker),
            "missing service export {marker}"
        );
    }

    for forbidden in [
        "async_graphql",
        "axum::",
        "#[server",
        "ForumCategoryAudienceVisibilityService",
        "ChannelService",
        "require_module_enabled",
        "forum_category_route_aliases",
        "INSERT INTO",
        "UPDATE ",
        "DELETE FROM",
        "StatusCode::",
    ] {
        assert!(
            !CATEGORY_ROUTE.contains(forbidden),
            "category route owner contains forbidden boundary marker {forbidden}"
        );
    }
}
