const MIGRATION: &str = include_str!(
    "../src/migrations/m20260824_000031_retire_forum_category_legacy_storage.rs"
);
const REGISTRY: &str = include_str!("../src/migrations/mod.rs");

#[test]
fn legacy_category_storage_retires_only_after_taxonomy_identity_cutover() {
    for marker in [
        "ensure_complete_taxonomy_ownership(manager).await?",
        "forum_category_taxonomy_binding::Entity::find()",
        "taxonomy_term::Entity::find()",
        "taxonomy_id == category.id",
        "TaxonomyTermKind::Category",
        "TaxonomyScopeType::Module",
        "term.scope_value != FORUM_SCOPE_VALUE",
        "ForumCategoryRouteAliases::Table",
        "ForumCategoryTranslations::Table",
        "ForumTranslationChanges::Table",
        "Intentionally irreversible",
    ] {
        assert!(MIGRATION.contains(marker), "missing retirement marker: {marker}");
    }

    let aliases = MIGRATION
        .find("ForumCategoryRouteAliases::Table")
        .expect("route aliases drop must exist");
    let translations = MIGRATION
        .find("ForumCategoryTranslations::Table")
        .expect("translations drop must exist");
    let changes = MIGRATION
        .find("ForumTranslationChanges::Table")
        .expect("translation changes drop must exist");
    assert!(aliases < translations && translations < changes);

    for forbidden in [
        "forum_category_translation::Entity",
        "forum_category_route_alias::Entity",
        "INSERT INTO forum_category_translations",
        "CREATE TABLE forum_category_translations",
    ] {
        assert!(
            !MIGRATION.contains(forbidden),
            "retirement migration must not rehydrate legacy donor storage: {forbidden}"
        );
    }

    assert!(REGISTRY.contains(
        "mod m20260824_000031_retire_forum_category_legacy_storage;"
    ));
    assert!(REGISTRY.contains(
        "Box::new(m20260824_000031_retire_forum_category_legacy_storage::Migration)"
    ));
    assert!(REGISTRY.contains(
        "MigrationDependencyDescriptor::new(\n            \"m20260824_000031_retire_forum_category_legacy_storage\",\n            vec![\"m20260823_000030_backfill_forum_categories_to_taxonomy\"]"
    ));
}
