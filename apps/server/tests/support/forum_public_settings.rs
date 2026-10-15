use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    PortError, SharedStaticModuleSettingsReader, StaticModuleSettingsReader,
    StaticModuleSettingsSnapshot,
};
use rustok_core::ModuleRuntimeExtensions;
use rustok_core::search_projection::{SearchProjectionSource, SearchProjectionSourceFactory};
use rustok_forum::{
    ForumSearchProjectionSourceFactory, ForumSearchResultEligibilityService, ForumSettingsProviders,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

struct DefaultForumSettingsReader;

#[async_trait]
impl StaticModuleSettingsReader for DefaultForumSettingsReader {
    async fn settings(
        &self, _tenant_id: Uuid, module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        assert_eq!(module_slug, "forum");
        Ok(Some(StaticModuleSettingsSnapshot {
            enabled: true,
            settings: serde_json::json!({}),
        }))
    }
}

pub fn default_settings_reader() -> SharedStaticModuleSettingsReader {
    SharedStaticModuleSettingsReader(Arc::new(DefaultForumSettingsReader))
}

pub fn default_settings() -> ForumSettingsProviders {
    ForumSettingsProviders::default().with_static_reader(default_settings_reader())
}

pub fn projection_source(db: DatabaseConnection) -> Arc<dyn SearchProjectionSource> {
    let mut extensions = ModuleRuntimeExtensions::default();
    extensions.insert(default_settings_reader());
    ForumSearchProjectionSourceFactory.build_with_extensions(db, &extensions)
}

pub fn eligibility_service(db: DatabaseConnection) -> ForumSearchResultEligibilityService {
    ForumSearchResultEligibilityService::new(db).with_settings_providers(default_settings())
}
