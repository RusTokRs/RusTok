//! Business logic wrapper for OAuth apps

use sea_orm::{entity::prelude::*, Condition, QueryFilter};
use uuid::Uuid;

pub use super::_entities::oauth_apps::{ActiveModel, Column, Entity, Model, Relation};

impl Entity {
    pub async fn find_active_by_client_id(
        db: &DatabaseConnection,
        client_id: Uuid,
    ) -> Result<Option<Model>, DbErr> {
        Entity::find()
            .filter(
                Condition::all()
                    .add(Column::ClientId.eq(client_id))
                    .add(Column::IsActive.eq(true)),
            )
            .one(db)
            .await
    }

    pub async fn find_by_tenant(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .all(db)
            .await
    }

    pub async fn find_active_by_tenant(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(
                Condition::all()
                    .add(Column::TenantId.eq(tenant_id))
                    .add(Column::IsActive.eq(true)),
            )
            .all(db)
            .await
    }
}

impl Model {
    pub fn is_active(&self) -> bool {
        self.is_active && self.revoked_at.is_none()
    }

    /// Parse scopes from JSONB field
    pub fn scopes_list(&self) -> Vec<String> {
        serde_json::from_value(self.scopes.clone()).unwrap_or_default()
    }

    /// Parse grant_types from JSONB field
    pub fn grant_types_list(&self) -> Vec<String> {
        serde_json::from_value(self.grant_types.clone()).unwrap_or_default()
    }

    /// Parse redirect_uris from JSONB field
    pub fn redirect_uris_list(&self) -> Vec<String> {
        serde_json::from_value(self.redirect_uris.clone()).unwrap_or_default()
    }

    /// Check if the app supports a specific grant type
    pub fn supports_grant_type(&self, grant_type: &str) -> bool {
        self.grant_types_list().iter().any(|gt| gt == grant_type)
    }
}
