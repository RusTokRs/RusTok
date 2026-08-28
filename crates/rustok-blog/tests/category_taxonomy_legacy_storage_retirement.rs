const MIGRATION: &str = include_str!(
    "../src/migrations/m20260828_000021_retire_blog_category_legacy_storage.rs"
);
const REGISTRY: &str = include_str!("../src/migrations/mod.rs");
const ENTITIES: &str = include_str!("../src/entities/mod.rs");
const BACKFILL: &str = include_str!(
    "../src/migrations/m20260824_000020_backfill_blog_categories_to_taxonomy.rs"
);

#[test]
fn legacy_category_storage_retires_only_after_taxonomy_identity_cutover() {
    for marker in [
        "ensure_complete_taxonomy_ownership(manager).await?",
        "blog_category_taxonomy_binding::Entity::find()",
        "taxonomy_term::Entity::find()",
        "taxonomy_id == category.id",
        "TaxonomyTermKind::Category",
        "TaxonomyScopeType::Module",
        "term.scope_value != BLOG_SCOPE_VALUE",
        "BlogCategoryTranslations::Table",
        "BlogTranslationChanges::Table",
        "Intentionally irreversible",
    ] {
        assert!(MIGRATION.contains(marker), "missing retirement marker: {marker}");
    }

    let translations = MIGRATION
        .find("BlogCategoryTranslations::Table")
        .expect("translations drop must exist");
    let changes = MIGRATION
        .find("BlogTranslationChanges::Table")
        .expect("translation changes drop must exist");
    assert!(translations < changes);

    for forbidden in [
        "blog_category_translation::Entity",
        "INSERT INTO blog_category_translations",
        "CREATE TABLE blog_category_translations",
        "CREATE TABLE blog_translation_changes",
    ] {
        assert!(
            !MIGRATION.contains(forbidden),
            "retirement migration must not rehydrate donor storage: {forbidden}"
        );
    }

    assert!(REGISTRY.contains(
        "mod m20260828_000021_retire_blog_category_legacy_storage;"
    ));
    assert!(REGISTRY.contains(
        "Box::new(m20260828_000021_retire_blog_category_legacy_storage::Migration)"
    ));
    assert!(REGISTRY.contains(
        "MigrationDependencyDescriptor::new(\n            \"m20260828_000021_retire_blog_category_legacy_storage\",\n            vec![\"m20260824_000020_backfill_blog_categories_to_taxonomy\"]"
    ));

    assert!(
        BACKFILL.contains("blog_category_translation::Entity::find()"),
        "historical backfill must still copy donor translations before the later retirement migration"
    );

    assert!(ENTITIES.contains("pub(crate) mod blog_category_translation;"));
    for forbidden in [
        "pub mod blog_category_translation;",
        "pub mod translation_change;",
        "BlogCategoryTranslation",
        "BlogTranslationChange",
    ] {
        assert!(
            !ENTITIES.contains(forbidden),
            "retired donor storage must not remain on the public Blog entity surface: {forbidden}"
        );
    }
}
