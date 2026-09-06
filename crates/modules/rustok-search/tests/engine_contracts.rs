use rustok_search::{
    SearchConnectorDescriptor, SearchEngineKind, SearchFilterPresetService, SearchRankingProfile,
    SearchSettingsRecord, SearchSettingsService,
};
use uuid::Uuid;

#[test]
fn postgres_is_default_connector() {
    let descriptor = SearchConnectorDescriptor::postgres_default();
    assert_eq!(descriptor.kind, SearchEngineKind::Postgres);
    assert!(descriptor.enabled);
    assert!(descriptor.default_engine);
}

#[test]
fn default_settings_fallback_to_postgres() {
    let tenant_id = Uuid::new_v4();
    let settings = SearchSettingsRecord::default_for_tenant(Some(tenant_id));

    assert_eq!(settings.tenant_id, Some(tenant_id));
    assert_eq!(settings.active_engine, SearchEngineKind::Postgres);
    assert_eq!(settings.fallback_engine, SearchEngineKind::Postgres);
}

#[test]
fn ranking_profile_validation_accepts_documented_surfaces() {
    SearchRankingProfile::validate_config(&serde_json::json!({
        "ranking_profiles": {
            "default": "balanced",
            "search_preview": "exact",
            "storefront_search": "catalog",
            "admin_global_search": "exact"
        }
    }))
    .expect("documented ranking profile surfaces should stay valid");
}

#[test]
fn filter_preset_validation_accepts_documented_ascii_contract() {
    SearchFilterPresetService::validate_config(&serde_json::json!({
        "filter_presets": {
            "default": [
                {
                    "key": "products",
                    "label": "Products",
                    "entity_types": ["product"],
                    "source_modules": ["commerce"],
                    "statuses": ["active"],
                    "ranking_profile": "catalog"
                }
            ],
            "admin_global_search": [
                {
                    "key": "content",
                    "label": "Content",
                    "entity_types": ["node"],
                    "source_modules": ["blog", "pages"],
                    "statuses": ["published"],
                    "ranking_profile": "content"
                }
            ]
        }
    }))
    .expect("documented preset contract should stay valid");
}

#[tokio::test]
async fn search_settings_save_rejects_invalid_ranking_profile_config() {
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("in-memory db");

    let error = SearchSettingsService::save(
        &db,
        None,
        SearchEngineKind::Postgres,
        SearchEngineKind::Postgres,
        serde_json::json!({
            "ranking_profiles": {
                "storefront_search": "bogus"
            }
        }),
    )
    .await
    .expect_err("invalid ranking profile config should fail before persistence");

    let message = error.to_string();
    assert!(message.contains("ranking_profiles.storefront_search"));
    assert!(message.contains("unsupported profile"));
}
