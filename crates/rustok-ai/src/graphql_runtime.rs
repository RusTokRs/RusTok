use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

pub const AI_GRAPHQL_CONTRIBUTION: rustok_api::graphql::GraphqlContributionDescriptor =
    rustok_api::graphql::GraphqlContributionDescriptor::new(
        Some("graphql::AiQuery"),
        Some("graphql::AiMutation"),
        Some("graphql::AiSubscription"),
        Some("graphql_runtime::attach_schema_data"),
    );

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

/// Single typed GraphQL context value owned by the AI capability.
#[cfg(feature = "server")]
#[derive(Clone)]
pub struct AiGraphqlRuntimeData {
    runtime: crate::AiHostRuntime,
    role_slug_provider: AiGraphqlRoleSlugProviderHandle,
}

#[cfg(feature = "server")]
impl AiGraphqlRuntimeData {
    pub fn runtime(&self) -> &crate::AiHostRuntime {
        &self.runtime
    }

    pub fn role_slug_provider(&self) -> &AiGraphqlRoleSlugProviderHandle {
        &self.role_slug_provider
    }
}

/// Capability-owned factory consumed by manifest-generated schema composition.
#[cfg(feature = "server")]
pub fn attach_schema_data(
    inputs: &rustok_api::graphql::GraphqlRuntimeInputs,
) -> Result<AiGraphqlRuntimeData, String> {
    let runtime = crate::ai_host_runtime_from_context(inputs.host())?;
    let role_slug_provider = AiGraphqlRoleSlugProviderHandle::new(Arc::new(
        SeaOrmAiGraphqlRoleSlugProvider::new(inputs.db_clone()),
    ));
    Ok(AiGraphqlRuntimeData {
        runtime,
        role_slug_provider,
    })
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
            "SELECT roles.slug FROM roles INNER JOIN user_roles ON user_roles.role_id = roles.id WHERE user_roles.user_id = $1 AND roles.tenant_id = $2 AND NOT EXISTS (SELECT 1 FROM oauth_apps WHERE oauth_apps.id = $1 AND oauth_apps.tenant_id = $2)"
        }
        DbBackend::MySql => {
            "SELECT roles.slug FROM roles INNER JOIN user_roles ON user_roles.role_id = roles.id WHERE user_roles.user_id = ? AND roles.tenant_id = ? AND NOT EXISTS (SELECT 1 FROM oauth_apps WHERE oauth_apps.id = ? AND oauth_apps.tenant_id = ?)"
        }
        DbBackend::Sqlite => {
            "SELECT roles.slug FROM roles INNER JOIN user_roles ON user_roles.role_id = roles.id WHERE user_roles.user_id = ?1 AND roles.tenant_id = ?2 AND NOT EXISTS (SELECT 1 FROM oauth_apps WHERE oauth_apps.id = ?1 AND oauth_apps.tenant_id = ?2)"
        }
    }
}

#[cfg(feature = "server")]
#[async_trait]
impl AiGraphqlRoleSlugProvider for SeaOrmAiGraphqlRoleSlugProvider {
    async fn load_role_slugs(&self, tenant_id: Uuid, user_id: Uuid) -> anyhow::Result<Vec<String>> {
        let backend = self.db.get_database_backend();
        let values = match backend {
            DbBackend::MySql => vec![
                user_id.into(),
                tenant_id.into(),
                user_id.into(),
                tenant_id.into(),
            ],
            DbBackend::Postgres | DbBackend::Sqlite => {
                vec![user_id.into(), tenant_id.into()]
            }
        };
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                backend,
                role_slug_query(backend),
                values,
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

    #[test]
    fn role_slug_query_excludes_oauth_app_subjects() {
        for backend in [DbBackend::Postgres, DbBackend::MySql, DbBackend::Sqlite] {
            let query = role_slug_query(backend);
            assert!(query.contains("NOT EXISTS"));
            assert!(query.contains("oauth_apps"));
        }
    }
}
