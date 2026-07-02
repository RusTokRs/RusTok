use async_trait::async_trait;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use rustok_ai::AiGraphqlRoleSlugProvider;

use crate::models::_entities::{roles, user_roles};

pub struct ServerAiGraphqlRoleSlugProvider {
    db: DatabaseConnection,
}

impl ServerAiGraphqlRoleSlugProvider {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AiGraphqlRoleSlugProvider for ServerAiGraphqlRoleSlugProvider {
    async fn load_role_slugs(&self, tenant_id: Uuid, user_id: Uuid) -> anyhow::Result<Vec<String>> {
        let assignments = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(user_id))
            .all(&self.db)
            .await?;
        if assignments.is_empty() {
            return Ok(Vec::new());
        }

        let roles = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(tenant_id))
            .filter(roles::Column::Id.is_in(assignments.into_iter().map(|item| item.role_id)))
            .all(&self.db)
            .await?;

        Ok(roles.into_iter().map(|item| item.slug).collect())
    }
}
