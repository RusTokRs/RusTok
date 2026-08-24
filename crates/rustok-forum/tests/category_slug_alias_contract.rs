const ROUTE: &str = include_str!("../src/services/category_route.rs");
const ALIAS: &str = include_str!("../src/services/category_route_alias.rs");
const CATEGORY_OWNER: &str = include_str!("../src/services/category_projection_owner.rs");
const MIGRATION: &str =
    include_str!("../src/migrations/m20260806_000026_add_forum_category_route_aliases.rs");
const MIGRATIONS_MOD: &str = include_str!("../src/migrations/mod.rs");

#[test]
fn migration_reserves_one_append_only_historical_route_namespace() {
    for marker in [
        "CREATE TABLE IF NOT EXISTS forum_category_route_aliases",
        "UNIQUE (tenant_id, locale, slug)",
        "FOREIGN KEY (tenant_id, category_id)",
        "forum category route aliases are append-only",
        "forum category route is reserved by alias",
        "forum category route alias conflicts with current route",
        "forum_category_translation_route_alias_guard",
        "forum_category_route_alias_insert_guard",
    ] {
        assert!(
            MIGRATION.contains(marker),
            "missing migration marker {marker}"
        );
    }
    assert!(MIGRATIONS_MOD.contains("m20260806_000026_add_forum_category_route_aliases"));
}

#[test]
fn every_public_category_slug_write_path_uses_the_route_owner() {
    for marker in [
        "ensure_current_route_key_available_in_tx(",
        "prepare_slug_rename_in_tx(",
        "record_slug_rename_alias_in_tx(",
        "FORUM_CATEGORY_RENAMED_ROUTE_REASON",
        "let previous_slug = normalize_required_slug(&existing_translation.slug)?;",
        "Some(name) =>",
        "normalize_required_slug(name)?",
        "if slug_changed",
        "publish_forum_projection_scope_direct_in_tx(",
    ] {
        assert!(
            CATEGORY_OWNER.contains(marker),
            "missing category write composition marker {marker}"
        );
    }

    assert_eq!(
        CATEGORY_OWNER
            .matches("ensure_current_route_key_available_in_tx(")
            .count(),
        2,
        "create and new-translation paths must both reserve the route key"
    );
}

#[test]
fn alias_owner_is_bounded_idempotent_and_never_reuses_history() {
    for marker in [
        "Historical route keys are never reusable",
        "pg_advisory_xact_lock",
        "keys.sort_unstable()",
        "ON CONFLICT (tenant_id, locale, slug) DO NOTHING",
        "MAX_FORUM_CATEGORY_ROUTE_ALIAS_REASON_LEN: usize = 500",
        "FORUM_CATEGORY_RENAMED_ROUTE_REASON",
        "load_exact_current_route_owners",
        "load_exact_category_route_aliases",
        "ForumError::CategoryRouteResolutionConflict",
    ] {
        assert!(
            ALIAS.contains(marker),
            "missing alias owner marker {marker}"
        );
    }
}

#[test]
fn resolver_combines_current_and_alias_candidates_without_authorizing_visibility() {
    for marker in [
        "pub alias_id: Option<Uuid>",
        "resolve_term_route_for_module",
        "route.alias_id.is_none()",
        "alias_id: route.alias_id",
    ] {
        assert!(
            ROUTE.contains(marker),
            "missing alias resolution marker {marker}"
        );
    }

    for forbidden in [
        "async_graphql",
        "axum::",
        "#[server",
        "ForumCategoryAudienceVisibilityService",
        "ChannelService",
        "require_module_enabled",
        "StatusCode::",
    ] {
        assert!(
            !ROUTE.contains(forbidden) && !ALIAS.contains(forbidden),
            "route owner contains forbidden transport/visibility marker {forbidden}"
        );
    }
}
