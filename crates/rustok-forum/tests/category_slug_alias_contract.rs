const ROUTE: &str = include_str!("../src/services/category_route.rs");
const CATEGORY_OWNER: &str = include_str!("../src/services/category_projection_owner.rs");
const CATEGORY_TAXONOMY_SYNC: &str = include_str!("../src/services/category_taxonomy_sync.rs");
const TAXONOMY_ROUTE_SYNC: &str = include_str!("../../rustok-taxonomy/src/owner_category_route_sync.rs");
const MIGRATION: &str =
    include_str!("../src/migrations/m20260806_000026_add_forum_category_route_aliases.rs");
const MIGRATIONS_MOD: &str = include_str!("../src/migrations/mod.rs");

#[test]
fn legacy_alias_migration_remains_available_for_upgrade_compatibility() {
    for marker in [
        "CREATE TABLE IF NOT EXISTS forum_category_route_aliases",
        "UNIQUE (tenant_id, locale, slug)",
        "FOREIGN KEY (tenant_id, category_id)",
    ] {
        assert!(
            MIGRATION.contains(marker),
            "missing compatibility migration marker {marker}"
        );
    }
    assert!(MIGRATIONS_MOD.contains("m20260806_000026_add_forum_category_route_aliases"));
}

#[test]
fn forum_route_reads_are_taxonomy_owned() {
    for marker in [
        "TaxonomyOwnerCategoryReader",
        "resolve_term_route_for_module(",
        "pub alias_id: Option<Uuid>",
        "forum_category_taxonomy_binding",
        "ensure_active_category",
    ] {
        assert!(ROUTE.contains(marker), "missing route marker {marker}");
    }
    for forbidden in [
        "category_route_alias.rs",
        "forum_category_route_aliases",
        "load_alias_route_candidates",
        "load_exact_current_route_owners",
    ] {
        assert!(
            !ROUTE.contains(forbidden),
            "Forum route reader still depends on legacy alias state: {forbidden}"
        );
    }
}

#[test]
fn forum_category_writes_delegate_alias_history_to_taxonomy() {
    assert!(
        CATEGORY_TAXONOMY_SYNC.contains("sync_module_category_with_owned_aliases_in_tx(")
    );
    assert!(CATEGORY_TAXONOMY_SYNC.contains("aliases: Vec::new()"));

    for forbidden in [
        "forum_category_route_aliases",
        "load_aliases_for_locale_in_tx",
        "record_slug_rename_alias_in_tx(",
        "prepare_slug_rename_in_tx(",
        "ensure_current_route_key_available_in_tx(",
    ] {
        assert!(
            !CATEGORY_OWNER.contains(forbidden) && !CATEGORY_TAXONOMY_SYNC.contains(forbidden),
            "Forum Category write path still owns route alias history: {forbidden}"
        );
    }
}

#[test]
fn taxonomy_route_sync_preserves_and_extends_append_only_history() {
    for marker in [
        "taxonomy_term_alias::Entity::find()",
        "taxonomy_term_translation::Entity::find()",
        "aliases.extend(std::mem::take(&mut input.aliases))",
        "if previous_slug != next_slug",
        "aliases.insert(previous_slug)",
        "sync_module_category_in_tx(txn, tenant_id, input).await",
    ] {
        assert!(
            TAXONOMY_ROUTE_SYNC.contains(marker),
            "missing Taxonomy-owned alias marker {marker}"
        );
    }
}

#[test]
fn route_owner_stays_below_transport_and_visibility_layers() {
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
            !ROUTE.contains(forbidden)
                && !CATEGORY_TAXONOMY_SYNC.contains(forbidden)
                && !TAXONOMY_ROUTE_SYNC.contains(forbidden),
            "route owner contains forbidden transport/visibility marker {forbidden}"
        );
    }
}
