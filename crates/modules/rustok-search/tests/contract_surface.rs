use rustok_search::{
    SearchConnectorDescriptor, SearchEngineKind, SearchFilterPresetService, SearchModule,
    SearchRankingProfile,
};

#[test]
fn implementation_plan_tracks_contract_and_error_policy_completion() {
    let plan = include_str!("../docs/implementation-plan.md");
    assert!(
        plan.contains("- [x] Expand capability matrix and contract tests"),
        "implementation plan must mark capability matrix and contract tests as complete"
    );
    assert!(
        plan.contains("- [x] Finalize search-facing error catalog and validation policy"),
        "implementation plan must mark error catalog and validation policy as complete"
    );
}

#[test]
fn readme_documents_capability_matrix_and_error_catalog() {
    let readme = include_str!("../README.md");
    assert!(
        readme.contains("## Capability matrix"),
        "README must document the search capability matrix"
    );
    assert!(
        readme.contains("## Error catalog and validation policy"),
        "README must document the search-facing error catalog"
    );
}

#[test]
fn module_available_engines_contract_is_postgres_first() {
    let module = SearchModule;
    let engines = module.available_engines();

    assert_eq!(
        engines,
        vec![SearchConnectorDescriptor::postgres_default()],
        "core search module should only expose the built-in postgres connector by default"
    );
}

#[test]
fn ranking_profile_surface_contract_stays_stable() {
    assert_eq!(
        SearchRankingProfile::known_surfaces(),
        &[
            SearchRankingProfile::CONFIG_DEFAULT_SURFACE,
            SearchRankingProfile::SEARCH_PREVIEW_SURFACE,
            SearchRankingProfile::STOREFRONT_SEARCH_SURFACE,
            SearchRankingProfile::ADMIN_GLOBAL_SEARCH_SURFACE,
        ]
    );
    assert_eq!(
        SearchRankingProfile::default_for_surface(
            SearchRankingProfile::ADMIN_GLOBAL_SEARCH_SURFACE
        ),
        SearchRankingProfile::Exact
    );
    assert_eq!(
        SearchRankingProfile::default_for_surface(SearchRankingProfile::STOREFRONT_SEARCH_SURFACE),
        SearchRankingProfile::Balanced
    );
}

#[test]
fn filter_presets_fall_back_to_default_surface_contract() {
    let config = serde_json::json!({
        "filter_presets": {
            "default": [
                {
                    "key": "all_content",
                    "label": "All content",
                    "entity_types": ["node"],
                    "source_modules": ["blog", "pages"],
                    "ranking_profile": "content"
                }
            ]
        }
    });

    let presets =
        SearchFilterPresetService::list(&config, SearchRankingProfile::ADMIN_GLOBAL_SEARCH_SURFACE);

    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0].key, "all_content");
    assert_eq!(
        presets[0].ranking_profile,
        Some(SearchRankingProfile::Content)
    );
}

#[test]
fn unknown_filter_preset_is_a_validation_error() {
    let error = SearchFilterPresetService::resolve(
        &serde_json::json!({
            "filter_presets": {
                "storefront_search": [
                    { "key": "products", "label": "Products" }
                ]
            }
        }),
        SearchRankingProfile::STOREFRONT_SEARCH_SURFACE,
        Some("missing"),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect_err("unknown preset should be rejected");

    assert!(
        error.to_string().contains("Unknown filter preset"),
        "unknown preset errors must stay caller-actionable"
    );
}

#[test]
fn search_engine_kind_roundtrip_contract_is_stable() {
    let known = [
        SearchEngineKind::Postgres,
        SearchEngineKind::Meilisearch,
        SearchEngineKind::Typesense,
        SearchEngineKind::Algolia,
    ];

    for kind in known {
        assert_eq!(SearchEngineKind::try_from_str(kind.as_str()), Some(kind));
        assert_eq!(
            SearchEngineKind::try_from_str(&kind.as_str().to_ascii_uppercase()),
            Some(kind),
            "engine parsing should remain case-insensitive"
        );
    }

    assert_eq!(
        SearchEngineKind::from_db_value("unknown"),
        SearchEngineKind::Postgres,
        "legacy or malformed persisted values must still degrade to postgres for db helper callers"
    );
}
