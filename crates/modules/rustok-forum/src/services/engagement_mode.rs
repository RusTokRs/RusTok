use serde_json::Value;
use uuid::Uuid;

use rustok_api::{
    PortError, PortErrorKind, SharedStaticModuleSettingsReader,
    SharedStaticModuleSettingsTransactionReader, StaticModuleSettingsSnapshot,
};
use sea_orm::DatabaseTransaction;

use crate::error::{ForumError, ForumResult};

pub const FORUM_MODULE_SLUG: &str = "forum";
pub const FORUM_REACTIONS_MODULE_SLUG: &str = "reactions";
#[cfg(test)]
pub const FORUM_USE_REACTIONS_SETTING: &str = "use_reactions";

pub type ForumSettings = crate::dto::ForumModuleSettings;

#[derive(Clone, Default)]
pub struct ForumSettingsProviders {
    static_reader: Option<SharedStaticModuleSettingsReader>,
    transactional_reader: Option<SharedStaticModuleSettingsTransactionReader>,
}

impl ForumSettingsProviders {
    pub fn with_static_readers(
        mut self,
        static_reader: SharedStaticModuleSettingsReader,
        transactional_reader: SharedStaticModuleSettingsTransactionReader,
    ) -> Self {
        self.static_reader = Some(static_reader);
        self.transactional_reader = Some(transactional_reader);
        self
    }

    /// Reads the Forum module settings inside the caller's transaction. A disabled module or
    /// a missing reader yields the default settings, so every flag takes its default value.
    pub(crate) async fn module_settings_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> ForumResult<ForumSettings> {
        let Some(reader) = self.transactional_reader.as_ref() else {
            return Ok(ForumSettings::default());
        };
        let snapshot = reader
            .settings_in_tx(txn, tenant_id, FORUM_MODULE_SLUG)
            .await
            .map_err(map_port_error)?;
        settings_from_snapshot(snapshot)
    }

    /// Non-transactional variant of [`Self::module_settings_in_tx`] for read-before-write checks.
    pub(crate) async fn module_settings(&self, tenant_id: Uuid) -> ForumResult<ForumSettings> {
        let Some(reader) = self.static_reader.as_ref() else {
            return Ok(ForumSettings::default());
        };
        let snapshot = reader
            .settings(tenant_id, FORUM_MODULE_SLUG)
            .await
            .map_err(map_port_error)?;
        settings_from_snapshot(snapshot)
    }

    /// Page size for topic lists when a transport request omits `per_page`.
    pub(crate) async fn default_topics_per_page(&self, tenant_id: Uuid) -> ForumResult<u64> {
        let settings = self.module_settings(tenant_id).await?;
        Ok(crate::dto::bounded_forum_read_limit(Some(u64::from(
            settings.topics_per_page,
        ))))
    }

    /// Page size for reply lists when a transport request omits `per_page`.
    pub(crate) async fn default_replies_per_page(&self, tenant_id: Uuid) -> ForumResult<u64> {
        let settings = self.module_settings(tenant_id).await?;
        Ok(crate::dto::bounded_forum_read_limit(Some(u64::from(
            settings.replies_per_page,
        ))))
    }
}

/// A disabled Forum module, or a missing row, yields the default settings.
fn settings_from_snapshot(
    snapshot: Option<StaticModuleSettingsSnapshot>,
) -> ForumResult<ForumSettings> {
    match snapshot {
        Some(snapshot) if snapshot.enabled => {
            Ok(parse_forum_settings(&snapshot.settings)?.unwrap_or_default())
        }
        _ => Ok(ForumSettings::default()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForumEngagementMode {
    InternalVotes,
    Reactions,
}

impl ForumEngagementMode {
    pub async fn resolve(providers: &ForumSettingsProviders, tenant_id: Uuid) -> ForumResult<Self> {
        let Some(reader) = providers.static_reader.as_ref() else {
            return Ok(Self::InternalVotes);
        };

        let forum_snapshot = reader
            .settings(tenant_id, FORUM_MODULE_SLUG)
            .await
            .map_err(map_port_error)?;

        let forum_settings = if forum_snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.enabled)
        {
            forum_snapshot
                .as_ref()
                .map(|snapshot| parse_forum_settings(&snapshot.settings))
                .transpose()?
                .flatten()
                .unwrap_or_default()
        } else {
            ForumSettings::default()
        };

        if !forum_settings.use_reactions {
            return Ok(Self::InternalVotes);
        }

        let reactions_enabled = reader
            .settings(tenant_id, FORUM_REACTIONS_MODULE_SLUG)
            .await
            .map_err(map_port_error)?
            .is_some_and(|snapshot| snapshot.enabled);

        Self::from_parts(true, reactions_enabled)
    }

    pub async fn resolve_in_tx(
        providers: &ForumSettingsProviders,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> ForumResult<Self> {
        let forum_settings = providers.module_settings_in_tx(txn, tenant_id).await?;

        if !forum_settings.use_reactions {
            return Ok(Self::InternalVotes);
        }

        let Some(reader) = providers.transactional_reader.as_ref() else {
            return Ok(Self::InternalVotes);
        };
        let reactions_enabled = reader
            .settings_in_tx(txn, tenant_id, FORUM_REACTIONS_MODULE_SLUG)
            .await
            .map_err(map_port_error)?
            .is_some_and(|snapshot| snapshot.enabled);

        Self::from_parts(true, reactions_enabled)
    }

    fn from_parts(use_reactions: bool, reactions_enabled: bool) -> ForumResult<Self> {
        if !use_reactions {
            return Ok(Self::InternalVotes);
        }

        if reactions_enabled {
            Ok(Self::Reactions)
        } else {
            Err(ForumError::capability_unavailable(
                "reactions",
                "FORUM_REACTIONS_CAPABILITY_UNAVAILABLE",
            ))
        }
    }

    pub fn require_internal_voting(self) -> ForumResult<()> {
        match self {
            Self::InternalVotes => Ok(()),
            Self::Reactions => Err(ForumError::InternalVotingDisabled),
        }
    }

    pub fn uses_reactions(self) -> bool {
        matches!(self, Self::Reactions)
    }

    pub fn is_internal_voting(self) -> bool {
        matches!(self, Self::InternalVotes)
    }
}

fn parse_forum_settings(value: &Value) -> ForumResult<Option<ForumSettings>> {
    serde_json::from_value::<ForumSettings>(value.clone())
        .map(Some)
        .map_err(|error| {
            ForumError::Validation(format!("Forum module settings are invalid: {error}"))
        })
}

fn map_port_error(error: PortError) -> ForumError {
    match error.kind {
        PortErrorKind::Unavailable | PortErrorKind::Timeout => ForumError::capability_failure(
            "static_module_settings",
            error.code,
            "Static module settings are temporarily unavailable",
            true,
        ),
        PortErrorKind::InvariantViolation => ForumError::Internal(rustok_core::Error::Internal(
            "Static module settings violated an owner invariant".to_string(),
        )),
        PortErrorKind::Validation
        | PortErrorKind::NotFound
        | PortErrorKind::Conflict
        | PortErrorKind::Forbidden => ForumError::Validation(
            "Static module settings request was rejected by its owner".to_string(),
        ),
    }
}

/// Test-only providers that keep the module defaults but set both posting cooldowns to `0`.
/// Tests that create several posts by one author, and are not about the cooldown, use this so the
/// soft-default cooldown does not reject the second create. Integration tests use the matching
/// helper in `tests/support/posting_cooldown.rs`.
#[cfg(test)]
pub(crate) fn providers_without_posting_cooldown() -> ForumSettingsProviders {
    use async_trait::async_trait;
    use rustok_api::{
        PortError, SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
        StaticModuleSettingsReader, StaticModuleSettingsTransactionReader,
    };
    use std::sync::Arc;

    struct ZeroCooldownSettings;

    fn snapshot(module_slug: &str) -> Option<StaticModuleSettingsSnapshot> {
        Some(StaticModuleSettingsSnapshot {
            enabled: module_slug == FORUM_MODULE_SLUG,
            settings: serde_json::json!({
                "rate_limit_new_topic_seconds": 0,
                "rate_limit_new_reply_seconds": 0,
            }),
        })
    }

    #[async_trait]
    impl StaticModuleSettingsReader for ZeroCooldownSettings {
        async fn settings(
            &self,
            _tenant_id: Uuid,
            module_slug: &str,
        ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
            Ok(snapshot(module_slug))
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
            Ok(snapshot(module_slug))
        }
    }

    let reader = Arc::new(ZeroCooldownSettings);
    ForumSettingsProviders::default().with_static_readers(
        SharedStaticModuleSettingsReader(reader.clone()),
        SharedStaticModuleSettingsTransactionReader(reader),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use rustok_api::{
        PortError, SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
        StaticModuleSettingsReader, StaticModuleSettingsSnapshot,
        StaticModuleSettingsTransactionReader,
    };
    use sea_orm::DatabaseTransaction;
    use uuid::Uuid;

    use super::{FORUM_USE_REACTIONS_SETTING, ForumEngagementMode, ForumSettingsProviders};

    struct EmptySettingsReader;

    #[async_trait]
    impl StaticModuleSettingsReader for EmptySettingsReader {
        async fn settings(
            &self,
            _tenant_id: Uuid,
            _module_slug: &str,
        ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
            Ok(None)
        }
    }

    #[async_trait]
    impl StaticModuleSettingsTransactionReader for EmptySettingsReader {
        async fn settings_in_tx(
            &self,
            _txn: &DatabaseTransaction,
            _tenant_id: Uuid,
            _module_slug: &str,
        ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
            Ok(None)
        }
    }

    fn providers() -> ForumSettingsProviders {
        let reader = Arc::new(EmptySettingsReader);
        ForumSettingsProviders::default().with_static_readers(
            SharedStaticModuleSettingsReader(reader.clone()),
            SharedStaticModuleSettingsTransactionReader(reader),
        )
    }

    #[tokio::test]
    async fn forum_defaults_to_internal_votes_without_an_override() {
        let mode = ForumEngagementMode::resolve(&providers(), Uuid::new_v4())
            .await
            .expect("mode");

        assert_eq!(mode, ForumEngagementMode::InternalVotes);
    }

    #[test]
    fn forum_setting_name_is_canonical() {
        assert_eq!(FORUM_USE_REACTIONS_SETTING, "use_reactions");
    }

    #[test]
    fn forum_setting_selects_reactions_only_when_shared_module_is_enabled() {
        assert_eq!(
            ForumEngagementMode::from_parts(false, true).expect("internal voting"),
            ForumEngagementMode::InternalVotes
        );
        assert_eq!(
            ForumEngagementMode::from_parts(true, true).expect("reactions"),
            ForumEngagementMode::Reactions
        );
        assert!(
            ForumEngagementMode::from_parts(true, false).is_err(),
            "selecting reactions without the shared module must fail closed"
        );
    }
}
