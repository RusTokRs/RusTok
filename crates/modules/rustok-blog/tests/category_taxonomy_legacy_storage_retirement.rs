const MIGRATION: &str = include_str!("../src/migrations/m20261029_000001_create_blog_schema.rs");
const REGISTRY: &str = include_str!("../src/migrations/mod.rs");
const ENTITIES: &str = include_str!("../src/entities/mod.rs");

#[test]
fn legacy_category_storage_is_retired_in_canonical_schema() {
    for forbidden in [
        "blog_category_translation::Entity",
        "INSERT INTO blog_category_translations",
        "CREATE TABLE blog_category_translations",
        "CREATE TABLE blog_translation_changes",
        "BlogCategoryTranslations",
        "BlogTranslationChanges",
    ] {
        assert!(
            !MIGRATION.contains(forbidden),
            "canonical schema must not rehydrate donor storage: {forbidden}"
        );
    }

    assert!(REGISTRY.contains("mod m20261029_000001_create_blog_schema;"));
    assert!(REGISTRY.contains("Box::new(m20261029_000001_create_blog_schema::Migration)"));
    assert!(REGISTRY.contains(
        "MigrationDependencyDescriptor::new(\n        \"m20261029_000001_create_blog_schema\""
    ));

    for forbidden in [
        "blog_category_translation",
        "pub mod translation_change;",
        "BlogCategoryTranslation",
        "BlogTranslationChange",
    ] {
        assert!(
            !ENTITIES.contains(forbidden),
            "retired donor storage must not remain on the Blog entity surface: {forbidden}"
        );
    }
}
