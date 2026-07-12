use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

#[cfg(feature = "server")]
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};

#[async_trait]
pub trait AiGraphqlRoleSlugProvider: Send + Sync {
    async fn load_role_slugs(&self, tenant_id: Uuid, user_id: Uuid) -> anyhow::Result<Vec<String>>;
}

#[derive(Clone)]
pub struct AiGraphqlRoleSlugProviderHandle {
    provider: Arc<dyn AiGraphqlRoleSlugProvider>,
}

impl AiGraphqlRoleSlugProviderHandle {
    pub fn new(provider: Arc<dyn AiGraphqlRoleSlugProvider>) -> Self {
        Self { provider }
    }

    pub async fn load_role_slugs(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<String>> {
        self.provider.load_role_slugs(tenant_id, user_id).await
    }
}

/// Deployment-neutral SeaORM implementation used by every host that exposes
/// the AI GraphQL surface. Keeping this lookup in the capability prevents a
/// host from becoming an AI-specific RBAC adapter.
#[cfg(feature = "server")]
pub struct SeaOrmAiGraphqlRoleSlugProvider {
    db: DatabaseConnection,
}

#[cfg(feature = "server")]
impl SeaOrmAiGraphqlRoleSlugProvider {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[cfg(feature = "server")]
fn role_slug_query(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => {
            "SELECT roles.slug FROM roles INNER JOIN user_roles ON user_roles.role_id = roles.id WHERE user_roles.user_id = $1 AND roles.tenant_id = $2"
        }
        DbBackend::MySql => {
            "SELECT roles.slug FROM roles INNER JOIN user_roles ON user_roles.role_id = roles.id WHERE user_roles.user_id = ? AND roles.tenant_id = ?"
        }
        DbBackend::Sqlite => {
            "SELECT roles.slug FROM roles INNER JOIN user_roles ON user_roles.role_id = roles.id WHERE user_roles.user_id = ?1 AND roles.tenant_id = ?2"
        }
    }
}

#[cfg(feature = "server")]
#[async_trait]
impl AiGraphqlRoleSlugProvider for SeaOrmAiGraphqlRoleSlugProvider {
    async fn load_role_slugs(&self, tenant_id: Uuid, user_id: Uuid) -> anyhow::Result<Vec<String>> {
        let backend = self.db.get_database_backend();
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                backend,
                role_slug_query(backend),
                [user_id.into(), tenant_id.into()],
            ))
            .await?;
        rows.into_iter()
            .map(|row| row.try_get::<String>("", "slug").map_err(Into::into))
            .collect()
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::role_slug_query;
    use sea_orm::DbBackend;

    #[test]
    fn role_slug_query_uses_backend_specific_placeholders() {
        assert!(role_slug_query(DbBackend::Postgres).contains("$1"));
        assert!(role_slug_query(DbBackend::Postgres).contains("$2"));
        assert!(!role_slug_query(DbBackend::Postgres).contains("?1"));

        assert!(role_slug_query(DbBackend::MySql).contains(" = ? "));
        assert!(!role_slug_query(DbBackend::MySql).contains("$1"));

        assert!(role_slug_query(DbBackend::Sqlite).contains("?1"));
        assert!(role_slug_query(DbBackend::Sqlite).contains("?2"));
    }
}
