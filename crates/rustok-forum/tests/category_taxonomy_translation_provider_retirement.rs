const SERVICES: &str = include_str!("../src/services/mod.rs");
const OWNER: &str = include_str!("../src/services/category_projection_owner.rs");
const IMPORT: &str = include_str!("../src/services/category_import.rs");
const LIFECYCLE: &str = include_str!("../src/services/category_lifecycle_owner.rs");
const LOCALES: &str = include_str!("../src/services/category_owner_locale_enumeration.rs");
const SERVER: &str = include_str!("../../../apps/server/src/services/module_event_dispatcher.rs");

#[test]
fn forum_category_translation_provider_is_retired_after_taxonomy_cutover() {
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

    // Search/read-model candidate enumeration still consumes this temporary mirror.
    // It is deliberately retained until that separate CAT-5 cutover lands.
    assert!(OWNER.contains("forum_category_translation::ActiveModel"));
    assert!(IMPORT.contains("forum_category_translation::ActiveModel"));
    assert!(OWNER.contains("sync_category_copy_in_tx"));
    assert!(IMPORT.contains("sync_category_copy_in_tx"));
}
