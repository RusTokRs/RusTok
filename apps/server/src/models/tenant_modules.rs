use sea_orm::prelude::*;

use rustok_core::generate_id;

use super::_entities::tenant_modules::{self, ActiveModel, Entity, Model};

impl ActiveModel {
    pub fn enable(tenant_id: Uuid, module_slug: &str) -> Self {
        Self {
            id: sea_orm::ActiveValue::Set(generate_id()),
            tenant_id: sea_orm::ActiveValue::Set(tenant_id),
            module_slug: sea_orm::ActiveValue::Set(module_slug.to_string()),
            enabled: sea_orm::ActiveValue::Set(true),
            settings: sea_orm::ActiveValue::Set(serde_json::json!({})),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
    }
}

impl Entity {
    pub async fn is_enabled(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<bool, DbErr> {
        let result = Self::find()
            .filter(tenant_modules::Column::TenantId.eq(tenant_id))
            .filter(tenant_modules::Column::ModuleSlug.eq(module_slug))
            .filter(tenant_modules::Column::Enabled.eq(true))
            .one(db)
            .await?;

        Ok(result.is_some())
    }

    pub async fn find_enabled(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<String>, DbErr> {
        let modules = Self::find()
            .filter(tenant_modules::Column::TenantId.eq(tenant_id))
            .filter(tenant_modules::Column::Enabled.eq(true))
            .all(db)
            .await?;

        Ok(modules.into_iter().map(|m| m.module_slug).collect())
    }

    pub async fn toggle(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        module_slug: &str,
        enabled: bool,
    ) -> Result<Model, DbErr> {
        let existing = Self::find()
            .filter(tenant_modules::Column::TenantId.eq(tenant_id))
            .filter(tenant_modules::Column::ModuleSlug.eq(module_slug))
            .one(db)
            .await?;

        match existing {
            Some(model) => {
                let mut active: ActiveModel = model.into();
                active.enabled = sea_orm::ActiveValue::Set(enabled);
                active.update(db).await
            }
            None => {
                let mut active = ActiveModel::enable(tenant_id, module_slug);
                active.enabled = sea_orm::ActiveValue::Set(enabled);
                active.insert(db).await
            }
        }
    }
}

pub async fn toggle(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    module_slug: &str,
    enabled: bool,
) -> Result<Model, DbErr> {
    Entity::toggle(db, tenant_id, module_slug, enabled).await
}

pub async fn find_enabled(db: &DatabaseConnection, tenant_id: Uuid) -> Result<Vec<String>, DbErr> {
    Entity::find_enabled(db, tenant_id).await
}
