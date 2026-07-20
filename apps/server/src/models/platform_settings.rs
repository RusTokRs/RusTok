use sea_orm::Set;
use sea_orm::prelude::*;

use rustok_core::generate_id;

use super::_entities::platform_settings;
pub use super::_entities::platform_settings::{ActiveModel, Column, Entity, Model};

impl ActiveModel {
    pub fn new(
        tenant_id: Uuid,
        category: impl Into<String>,
        settings: serde_json::Value,
        updated_by: Option<Uuid>,
    ) -> Self {
        Self {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            category: Set(category.into()),
            settings: Set(settings),
            schema_version: Set(1),
            updated_by: Set(updated_by),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
    }
}

impl Entity {
    pub async fn find_by_category(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        category: &str,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(platform_settings::Column::TenantId.eq(tenant_id))
            .filter(platform_settings::Column::Category.eq(category))
            .one(db)
            .await
    }

    pub async fn find_all_for_tenant(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(platform_settings::Column::TenantId.eq(tenant_id))
            .all(db)
            .await
    }
}
