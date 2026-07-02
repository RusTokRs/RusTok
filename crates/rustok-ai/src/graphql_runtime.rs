use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

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
