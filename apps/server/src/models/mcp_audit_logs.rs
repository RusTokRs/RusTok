use sea_orm::{QueryFilter, QueryOrder, QuerySelect, entity::prelude::*};

pub use super::_entities::mcp_audit_logs::{ActiveModel, Column, Entity, Model, Relation};

impl Entity {
    pub async fn find_by_tenant(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        limit: u64,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::TenantId.eq(tenant_id))
            .order_by_desc(Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
    }
}
