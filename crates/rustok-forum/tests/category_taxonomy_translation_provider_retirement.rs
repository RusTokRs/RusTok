const SERVICES: &str = include_str!("../src/services/mod.rs");
const OWNER: &str = include_str!("../src/services/category_projection_owner.rs");
const IMPORT: &str = include_str!("../src/services/category_import.rs");
const LIFECYCLE: &str = include_str!("../src/services/category_lifecycle_owner.rs");
const LOCALES: &str = include_str!("../src/services/category_owner_locale_enumeration.rs");
const SEARCH: &str = include_str!("../src/search_projection.rs");
const READ_MODEL: &str = include_str!("../src/services/read_model_owner.rs");
const SERVER: &str = include_str!("../../../apps/server/src/services/module_event_dispatcher.rs");

#[test]
fn forum_category_translation_provider_and_mirror_writes_are_retired_after_taxonomy_cutover() {
    for forbidden in [
        "mod category_translation_target;",
        "mod category_translation_progress;",
        "mod category_translation_evidence;",
        "ForumCategoryTranslationTargetProvider",
    ] {
        assert!(
            !SERVICES.contains(forbidden),
            "Forum services must not expose retired Translation ownership marker {forbidden}"
        );
    }

    assert!(SERVER.contains("TaxonomyTranslationTargetProvider"));
    assert!(SERVER.contains("Taxonomy translation target provider registration failed"));
    assert!(!SERVER.contains("ForumCategoryTranslationTargetProvider"));
    assert!(!SERVER.contains("Forum category translation target provider registration failed"));

    assert!(LOCALES.contains("TaxonomyOwnerCategoryReader"));
    assert!(LOCALES.contains("projection.available_locales"));
    assert!(!LOCALES.contains("self.inner.available_locales_for_categories"));

    assert!(!LIFECYCLE.contains("category_translation_evidence"));
    assert!(!IMPORT.contains("category_translation_evidence"));
    assert!(!IMPORT.contains("ensure_current_route_key_available_in_tx"));

    assert!(OWNER.contains("sync_category_copy_in_tx"));
    assert!(IMPORT.contains("sync_category_copy_in_tx"));
    assert!(!OWNER.contains("forum_category_translation"));
    assert!(!IMPORT.contains("forum_category_translation"));

    assert!(SEARCH.contains("TaxonomyOwnerCategoryReader"));
    assert!(!SEARCH.contains("forum_category_translation::Entity::find()"));
    assert!(READ_MODEL.contains("TaxonomyOwnerCategoryReader"));
    assert!(!READ_MODEL.contains("forum_category_translation"));
}
