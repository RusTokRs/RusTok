use chrono::Utc;
use sea_orm::prelude::*;
use sea_orm::EntityTrait;

use rustok_core::generate_id;

pub use super::_entities::tenants::ActiveModel;
pub use super::_entities::tenants::Entity;
pub use super::_entities::tenants::Model;
use super::_entities::tenants::{self};

impl Model {
    pub fn is_enabled(&self) -> bool {
        self.is_active
    }
}

impl ActiveModel {
    pub fn new(name: &str, slug: &str) -> Self {
        let now = Utc::now().fixed_offset();

        Self {
            id: sea_orm::ActiveValue::Set(generate_id()),
            name: sea_orm::ActiveValue::Set(name.to_string()),
            slug: sea_orm::ActiveValue::Set(slug.to_string()),
            domain: sea_orm::ActiveValue::NotSet,
            settings: sea_orm::ActiveValue::Set(serde_json::json!({})),
            default_locale: sea_orm::ActiveValue::Set("en".to_string()),
            is_active: sea_orm::ActiveValue::Set(true),
            created_at: sea_orm::ActiveValue::Set(now),
            updated_at: sea_orm::ActiveValue::Set(now),
        }
    }
}

impl Entity {
    pub async fn find_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<Model>, DbErr> {
        <Self as EntityTrait>::find_by_id(id).one(db).await
    }

    pub async fn find_by_slug(db: &DatabaseConnection, slug: &str) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(tenants::Column::Slug.eq(slug))
            .one(db)
            .await
    }

    pub async fn find_by_domain(
        db: &DatabaseConnection,
        domain: &str,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(tenants::Column::Domain.eq(domain))
            .one(db)
            .await
    }

    pub async fn find_active(db: &DatabaseConnection) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(tenants::Column::IsActive.eq(true))
            .all(db)
            .await
    }

    pub async fn find_or_create(
        db: &DatabaseConnection,
        name: &str,
        slug: &str,
        domain: Option<&str>,
    ) -> Result<Model, DbErr> {
        if let Some(existing) = Self::find_by_slug(db, slug).await? {
            return Ok(existing);
        }

        let mut tenant = ActiveModel::new(name, slug);
        tenant.domain = sea_orm::ActiveValue::Set(domain.map(|value| value.to_string()));
        tenant.insert(db).await
    }
}
