//! Test-only Forum settings providers with the posting cooldowns disabled.
//!
//! Many integration tests create several topics or replies by the same user back to back.
//! With the soft defaults, the per-author cooldown would reject the second create. These
//! providers return the module defaults with both `rate_limit_*` values set to `0`. Every other
//! setting keeps its default, because the providers carry no stored tenant values.
//!
//! `zero_cooldown_providers_with` adds extra stored values, for tests that need one policy setting
//! on top of the disabled cooldowns.
//!
//! Tests whose subject is the cooldown itself must not use this module.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    PortError, SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
    StaticModuleSettingsReader, StaticModuleSettingsSnapshot,
    StaticModuleSettingsTransactionReader,
};
use rustok_forum::ForumSettingsProviders;
use sea_orm::DatabaseTransaction;
use uuid::Uuid;

const FORUM_MODULE_SLUG: &str = "forum";

struct ZeroCooldownSettings {
    extra: serde_json::Value,
}

fn snapshot(module_slug: &str, extra: &serde_json::Value) -> Option<StaticModuleSettingsSnapshot> {
    let mut settings = serde_json::json!({
        "use_reactions": false,
        "rate_limit_new_topic_seconds": 0,
        "rate_limit_new_reply_seconds": 0,
    });
    if let (Some(base), Some(extra)) = (settings.as_object_mut(), extra.as_object()) {
        base.extend(extra.iter().map(|(key, value)| (key.clone(), value.clone())));
    }
    Some(StaticModuleSettingsSnapshot {
        enabled: module_slug == FORUM_MODULE_SLUG,
        settings,
    })
}

#[async_trait]
impl StaticModuleSettingsReader for ZeroCooldownSettings {
    async fn settings(
        &self,
        _tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        Ok(snapshot(module_slug, &self.extra))
    }
}

#[async_trait]
impl StaticModuleSettingsTransactionReader for ZeroCooldownSettings {
    async fn settings_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        Ok(snapshot(module_slug, &self.extra))
    }
}

/// Providers for `TopicService` and `ReplyService` in tests that are not about the cooldown.
pub fn zero_cooldown_providers() -> ForumSettingsProviders {
    zero_cooldown_providers_with(serde_json::json!({}))
}

/// Like [`zero_cooldown_providers`], with `extra` settings merged over the test values.
pub fn zero_cooldown_providers_with(extra: serde_json::Value) -> ForumSettingsProviders {
    let reader = Arc::new(ZeroCooldownSettings { extra });
    ForumSettingsProviders::default().with_static_readers(
        SharedStaticModuleSettingsReader(reader.clone()),
        SharedStaticModuleSettingsTransactionReader(reader),
    )
}
