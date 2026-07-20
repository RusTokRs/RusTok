use rustok_mcp::McpActorType;
use sea_orm::{Condition, QueryFilter, QueryOrder, entity::prelude::*};

pub use super::_entities::mcp_clients::{ActiveModel, Column, Entity, Model, Relation};

impl Entity {
    pub async fn find_by_tenant(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::TenantId.eq(tenant_id))
            .order_by_desc(Column::CreatedAt)
            .all(db)
            .await
    }

    pub async fn find_active_by_client_key(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        client_key: Uuid,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(
                Condition::all()
                    .add(Column::TenantId.eq(tenant_id))
                    .add(Column::ClientKey.eq(client_key))
                    .add(Column::IsActive.eq(true))
                    .add(Column::RevokedAt.is_null()),
            )
            .one(db)
            .await
    }
}

impl Model {
    pub fn is_active(&self) -> bool {
        self.is_active && self.revoked_at.is_none()
    }

    pub fn actor_type(&self) -> McpActorType {
        match self.actor_type.as_str() {
            "human_user" => McpActorType::HumanUser,
            "service_client" => McpActorType::ServiceClient,
            "model_agent" => McpActorType::ModelAgent,
            _ => McpActorType::ServiceClient,
        }
    }
}
