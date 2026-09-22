use sea_orm::{DatabaseConnection, DatabaseTransaction};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use rustok_api::{tenant_module_settings, tenant_module_settings_in_tx};

use crate::error::{ForumError, ForumResult};

pub const FORUM_MODULE_SLUG: &str = "forum";
pub const FORUM_REACTIONS_MODULE_SLUG: &str = "reactions";
pub const FORUM_USE_REACTIONS_SETTING: &str = "use_reactions";

#[derive(Debug, Deserialize, Default)]
struct ForumSettings {
    #[serde(default)]
    use_reactions: bool,
}

#[derive(Clone, Copy, Debug, Eq, Partiaimpl ForumEngagementMode {
    pub async fn resolve(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> ForumResult<Self> {
        let forum_settings = tenant_module_settings(db, tenant_id, FORUM_MODULE_SLUG)
            .await?
            .as_ref()
            .map(parse_forum_settings)
            .transpose()?
            .flatten()
            .unwrap_or_default();

        if !forum_settings.use_reactions {
            return Ok(Self::InternalVotes);
        }

        let reactions_enabled =
            tenant_module_settings(db, tenant_id, FORUM_REACTIONS_MODULE_SLUG)
                .await?
                .is_some();

        Self::from_parts(true, reactions_enabled)
    }

    pub async fn resolve_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> ForumResult<Self> {
        let forum_settings = tenant_module_settings_in_tx(txn, tenant_id, FORUM_MODULE_SLUG)
            .await?
            .as_ref()
            .map(parse_forum_settings)
            .transpose()?
            .flatten()
            .unwrap_or_default();

        let reactions_enabled =
            tenant_module_settings_in_tx(txn, tenant_id, FORUM_REACTIONS_MODULE_SLUG)
                .await?
                .is_some();

        Self::from_parts(forum_settings.use_reactions, reactions_enabled)
    }

    fn from_parts(
        use_reactions: bool,
        reactions_enabled: bool,
    ) -> ForumResult<Self> {
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
        .map_err(|error| ForumError::Validation(format!(
            "Forum module settings are invalid: {error}"
        )))
}

alue::as_bool)
            .unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{ForumEngagementMode, FORUM_USE_REACTIONS_SETTING};
    
    #[tokio::test]
    async fn forum_defaults_to_internal_votes_without_an_override() {
        use sea_orm::{ConnectionTrait, Database};
        let db = Database::connect("sqlite::memory:").await.expect("db");
        db.execute_unprepared(
            "CREATE TABLE tenant_modules (
                id TEXT NOT NULL PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                module_slug TEXT NOT NULL,
                enabled BOOLEAN NOT NULL,
                settings TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .await
        .expect("schema");

        let mode = ForumEngagementMode::resolve(&db, uuid::Uuid::new_v4())
            .await
            .expect("mode");
        assert_eq!(mode, ForumEngagementMode::InternalVotes);
    }

    #[test]
    fn forum_setting_uses_canonical_key() {
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
#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database};

    use super::{ForumEngagementMode, FORUM_USE_REACTIONS_SETTING};

    #[tokio::test]
    async fn forum_defaults_to_internal_votes_without_an_override() {
        let db = Database::connect("sqlite::memory:").await.expect("db");
        db.execute_unprepared(
            "CREATE TABLE tenant_modules (
                id TEXT NOT NULL PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                module_slug TEXT NOT NULL,
                enabled BOOLEAN NOT NULL,
                settings TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .await
        .expect("schema");

        let mode = ForumEngagementMode::resolve(&db, uuid::Uuid::new_v4())
            .await
            .expect("mode");

        assert_eq!(mode, ForumEngagementMode::InternalVotes);
    }

    #[test]
    fn forum_setting_name_is_stable() {
        assert_eq!(FORUM_USE_REACTIONS_SETTING, "use_reactions");
    }

    #[test]
    fn forum_setting_selects_reactions_only_when_owner_module_is_enabled() {
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
