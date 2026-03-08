//! Business logic wrapper for OAuth tokens

use sea_orm::{entity::prelude::*, Condition, QueryFilter};
use uuid::Uuid;

pub use super::_entities::oauth_tokens::{ActiveModel, Column, Entity, Model, Relation};

impl Entity {
    pub async fn find_active_by_hash(
        db: &DatabaseConnection,
        token_hash: &str,
    ) -> Result<Option<Model>, DbErr> {
        Entity::find()
            .filter(
                Condition::all()
                    .add(Column::TokenHash.eq(token_hash))
                    .add(Column::RevokedAt.is_null()),
            )
            .one(db)
            .await
    }

    pub async fn find_active_by_app(
        db: &DatabaseConnection,
        app_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(
                Condition::all()
                    .add(Column::AppId.eq(app_id))
                    .add(Column::TenantId.eq(tenant_id))
                    .add(Column::RevokedAt.is_null()),
            )
            .all(db)
            .await
    }

    pub async fn count_active_by_app(db: &DatabaseConnection, app_id: Uuid) -> Result<u64, DbErr> {
        Entity::find()
            .filter(
                Condition::all()
                    .add(Column::AppId.eq(app_id))
                    .add(Column::RevokedAt.is_null()),
            )
            .count(db)
            .await
    }
}

impl Model {
    pub fn is_active(&self) -> bool {
        self.revoked_at.is_none()
    }

    pub fn scopes_list(&self) -> Vec<String> {
        serde_json::from_value(self.scopes.clone()).unwrap_or_default()
    }
}
