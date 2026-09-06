const LIB: &str = include_str!("../src/lib.rs");
const OWNER: &str = include_str!("../src/services/category_owner.rs");
const SERVER: &str = include_str!("../../../apps/server/src/services/module_event_dispatcher.rs");

#[test]
fn blog_category_translation_provider_is_retired_after_taxonomy_cutover() {
    for forbidden in [
        "pub mod translation_target;",
        "mod translation_target_tests;",
        "BlogCategoryTranslationTargetProvider",
    ] {
        assert!(
            !LIB.contains(forbidden),
            "Blog public surface must not expose retired Translation ownership marker {forbidden}"
        );
    }

    assert!(SERVER.contains("TaxonomyTranslationTargetProvider"));
    assert!(SERVER.contains("Taxonomy translation target provider registration failed"));
    assert!(SERVER.contains("blog_public_comments_snapshot::register"));
    assert!(!SERVER.contains("BlogCategoryTranslationTargetProvider"));
    assert!(!SERVER.contains("Blog category translation target provider registration failed"));

    assert!(OWNER.contains("TaxonomyOwnerCategoryReader"));
    assert!(OWNER.contains("load_category_response"));
}
