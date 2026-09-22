use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, EntityTrait,
    QueryFilter, QuerySelect,
};
use serde_json::Value;
use uuid::Uuid;

use rustok_tenant::entities::tenant_module;

use crate::error::{ForumError, ForumResult};

pub const FORUM_MODULE_SLUG: &str = "forum";
pub const FORUM_REACTIONS_MODULE_SLUG: &str = "reactions";
pub const FORUM_USE_REACTIONS_SETTING: &str = "useReactions";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForumEngagementMode {
    InternalVotes,
    Reactions,
}

impl ForumEngagementMode {
    pub async fn resolve(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> ForumResult<Self> {
        let forum_module = match tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .filter(tenant_module::Column::ModuleSlug.eq(FORUM_MODULE_SLUG))
            .one(db)
            .await
        {
            Ok(module) => module,
            Err(err) if is_missing_tenant_modules_error(&err) => return Ok(Self::InternalVotes),
            Err(err) => return Err(ForumError::Database(err)),
        };

        if !forum_use_reactions(forum_module.as_ref().map(|module| &module.settings)) {
            return Ok(Self::InternalVotes);
        }

        let reactions_enabled = match tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .filter(tenant_module::Column::ModuleSlug.eq(FORUM_REACTIONS_MODULE_SLUG))
            .filter(tenant_module::Column::Enabled.eq(true))
            .one(db)
            .await
        {
            Ok(module) => module.is_some(),
            Err(err) if is_missing_tenant_modules_error(&err) => false,
            Err(err) => return Err(ForumError::Database(err)),
        };

        Self::from_parts(true, reactions_enabled)
    }

    pub async fn resolve_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> ForumResult<Self> {
        let forum_query = tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .filter(tenant_module::Column::ModuleSlug.eq(FORUM_MODULE_SLUG));

        let reaction_query = tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .filter(tenant_module::Column::ModuleSlug.eq(FORUM_REACTIONS_MODULE_SLUG))
            .filter(tenant_module::Column::Enabled.eq(true));

        let (forum_module, reactions_module) = match txn.get_database_backend() {
            DbBackend::Sqlite => {
                let forum_module = match forum_query.one(txn).await {
                    Ok(m) => m,
                    Err(err) if is_missing_tenant_modules_error(&err) => return Ok(Self::InternalVotes),
                    Err(err) => return Err(ForumError::Database(err)),
                };
                let reactions_module = match reaction_query.one(txn).await {
                    Ok(m) => m,
                    Err(err) if is_missing_tenant_modules_error(&err) => None,
                    Err(err) => return Err(ForumError::Database(err)),
                };
                (forum_module, reactions_module)
            }
            DbBackend::Postgres | DbBackend::MySql => {
                let forum_module = match forum_query.lock_shared().one(txn).await {
                    Ok(m) => m,
                    Err(err) if is_missing_tenant_modules_error(&err) => return Ok(Self::InternalVotes),
                    Err(err) => return Err(ForumError::Database(err)),
                };
                let reactions_module = match reaction_query.lock_shared().one(txn).await {
                    Ok(m) => m,
                    Err(err) if is_missing_tenant_modules_error(&err) => None,
                    Err(err) => return Err(ForumError::Database(err)),
                };
                (forum_module, reactions_module)
            }
            backend => {
                return Err(ForumError::Database(sea_orm::DbErr::Custom(format!(
                    "forum engagement mode locking is unsupported for {backend:?}"
                ))));
            }
        };

        Self::from_parts(
            forum_use_reactions(forum_module.as_ref().map(|module| &module.settings)),
            reactions_module.is_some(),
        )
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

fn forum_use_reactions(settings: Option<&Value>) -> bool {
    match settings {
        Some(Value::Object(object)) => object
            .get(FORUM_USE_REACTIONS_SETTING)
            .and_then(Value::as_bool)
            .unwrap_or(false),
        Some(Value::String(raw)) => serde_json::from_str::<Value>(raw)
            .ok()
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|object| object.get(FORUM_USE_REACTIONS_SETTING))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        _ => false,
    }
}

fn is_missing_tenant_modules_error(err: &sea_orm::DbErr) -> bool {
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("no such table: tenant_modules")
        || msg.contains("relation \"tenant_modules\" does not exist")
        || msg.contains("relation 'tenant_modules' does not exist")
        || msg.contains("table 'tenant_modules' doesn't exist")
        || msg.contains("no such column: tenant_modules.created_at")
        || msg.contains("no such column: tenant_modules.updated_at")
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
        assert_eq!(FORUM_USE_REACTIONS_SETTING, "useReactions");
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
