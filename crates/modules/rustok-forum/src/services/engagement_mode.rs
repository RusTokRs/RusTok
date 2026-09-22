use sea_orm::{
    ColumnTrait, DatabaseTransaction, DbBackend, EntityTrait, QueryFilter, QuerySelect,
};
use uuid::Uuid;

use rustok_tenant::entities::tenant_module;

use crate::error::{ForumError, ForumResult};

pub const FORUM_REACTIONS_MODULE_SLUG: &str = "reactions";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForumEngagementMode {
    InternalVotes,
    Reactions,
}

impl ForumEngagementMode {
    pub async fn resolve_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> ForumResult<Self> {
        let query = tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .filter(tenant_module::Column::ModuleSlug.eq(FORUM_REACTIONS_MODULE_SLUG));

        let module = match txn.get_database_backend() {
            DbBackend::Sqlite => query.one(txn).await?,
            DbBackend::Postgres | DbBackend::MySql => query.lock_shared().one(txn).await?,
            backend => {
                return Err(ForumError::Database(sea_orm::DbErr::Custom(format!(
                    "forum engagement mode locking is unsupported for {backend:?}"
                ))));
            }
        };

        Ok(if module.is_some_and(|module| module.enabled) {
            Self::Reactions
        } else {
            Self::InternalVotes
        })
    }

    pub fn require_internal_voting(self) -> ForumResult<()> {
        match self {
            Self::InternalVotes => Ok(()),
            Self::Reactions => Err(ForumError::ReactionsEnabled),
        }
    }

    pub fn uses_reactions(self) -> bool {
        matches!(self, Self::Reactions)
    }
}

#[cfg(test)]
mod tests {
    use super::ForumEngagementMode;

    #[test]
    fn internal_votes_are_allowed_only_in_vote_mode() {
        assert!(ForumEngagementMode::InternalVotes
            .require_internal_voting()
            .is_ok());
        assert!(ForumEngagementMode::Reactions
            .require_internal_voting()
            .is_err());
    }

    #[test]
    fn reaction_mode_reports_reactions_as_active() {
        assert!(!ForumEngagementMode::InternalVotes.uses_reactions());
        assert!(ForumEngagementMode::Reactions.uses_reactions());
    }
}
