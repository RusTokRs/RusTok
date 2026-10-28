use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortError, StaticModuleSettingsReader, StaticModuleSettingsSnapshot};
use rustok_forum::{ForumSettingsProviders, ForumTopicListOrder};
use uuid::Uuid;

struct TenantSortReader {
    tenant_id: Uuid,
    value: String,
}

#[async_trait]
impl StaticModuleSettingsReader for TenantSortReader {
    async fn settings(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        if module_slug != "forum" {
            return Ok(None);
        }
        let sort = if tenant_id == self.tenant_id {
            self.value.as_str()
        } else {
            "latest_reply"
        };
        Ok(Some(StaticModuleSettingsSnapshot {
            enabled: Some(true),
            settings: serde_json::json!({ "default_topic_sort": sort }),
        }))
    }
}

#[tokio::test]
async fn the_tenant_default_order_is_resolved_from_the_static_owner() {
    let tenant_id = Uuid::new_v4();
    let foreign_tenant = Uuid::new_v4();
    let settings = ForumSettingsProviders::default().with_static_reader(Arc::new(
        TenantSortReader {
            tenant_id,
            value: "top".to_owned(),
        },
    ));

    assert_eq!(
        settings.default_topic_order(tenant_id).await.unwrap(),
        ForumTopicListOrder::Top
    );
    assert_eq!(
        settings.default_topic_order(foreign_tenant).await.unwrap(),
        ForumTopicListOrder::Latest
    );
    assert_eq!(
        ForumSettingsProviders::default()
            .default_topic_order(tenant_id)
            .await
            .unwrap(),
        ForumTopicListOrder::Latest
    );
}

#[tokio::test]
async fn invalid_persisted_order_does_not_fall_back_to_latest() {
    let tenant_id = Uuid::new_v4();
    let settings = ForumSettingsProviders::default().with_static_reader(Arc::new(
        TenantSortReader {
            tenant_id,
            value: "views".to_owned(),
        },
    ));
    assert!(settings.default_topic_order(tenant_id).await.is_err());
}
