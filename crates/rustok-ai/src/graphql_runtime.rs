use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

#[cfg(feature = "server")]
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TryGetable};

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
#[async_trait]
impl AiGraphqlRoleSlugProvider for SeaOrmAiGraphqlRoleSlugProvider {
    async fn load_role_slugs(&self, tenant_id: Uuid, user_id: Uuid) -> anyhow::Result<Vec<String>> {
        let backend = self.db.get_database_backend();
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                backend,
                "SELECT roles.slug FROM roles INNER JOIN user_roles ON user_roles.role_id = roles.id WHERE user_roles.user_id = ?1 AND roles.tenant_id = ?2",
                [user_id.into(), tenant_id.into()],
            ))
            .await?;
        rows.into_iter()
            .map(|row| row.try_get::<String>("", "slug").map_err(Into::into))
            .collect()
    }
}
