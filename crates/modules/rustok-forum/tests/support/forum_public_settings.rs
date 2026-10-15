use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    PortError, SharedStaticModuleSettingsReader, StaticModuleSettingsReader,
    StaticModuleSettingsSnapshot,
};
use rustok_forum::ForumSettingsProviders;
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

pub fn default_public_settings() -> ForumSettingsProviders {
    ForumSettingsProviders::default().with_static_reader(default_settings_reader())
}
