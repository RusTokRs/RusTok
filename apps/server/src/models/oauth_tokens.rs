//! Business logic wrapper for OAuth tokens

use sea_orm::{
    ColumnTrait, Condition, EntityTrait, QueryFilter, entity::prelude::*, sea_query::Expr,
};
use uuid::Uuid;

pub use super::_entities::oauth_tokens::{ActiveModel, Column, Entity, Model, Relation};

impl Entity {
    /// Atomically reserve an active token for one-shot refresh or revocation.
    ///
    /// A conditional UPDATE guarantees that concurrent refresh attempts cannot
    /// both observe the same token as active and mint multiple replacement
    /// token families. The returned model is an in-memory lease; the database
    /// row remains revoked before the caller issues a replacement.
    pub async fn find_active_by_hash(
        db: &DatabaseConnection,
        token_hash: &str,
        app_id: Uuid,
    ) -> Result<Option<Model>, DbErr> {
        let now = chrono::Utc::now();
        let reserved = Entity::update_many()
            .col_expr(Column::RevokedAt, Expr::value(now))
            .col_expr(Column::LastUsedAt, Expr::value(now))
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .filter(
                Condition::all()
                    .add(Column::TokenHash.eq(token_hash))
                    .add(Column::AppId.eq(app_id))
                    .add(Column::RevokedAt.is_null())
                    .add(Column::ExpiresAt.gt(now)),
            )
            .exec(db)
            .await?;

        if reserved.rows_affected != 1 {
            return Ok(None);
        }

        let mut token = Entity::find()
            .filter(Column::TokenHash.eq(token_hash))
            .filter(Column::AppId.eq(app_id))
            .one(db)
            .await?;
        if let Some(token) = token.as_mut() {
            token.revoked_at = None;
        }
        Ok(token)
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
                    .add(Column::RevokedAt.is_null())
                    .add(Column::ExpiresAt.gt(chrono::Utc::now())),
            )
            .all(db)
            .await
    }

    pub async fn count_active_by_app(db: &DatabaseConnection, app_id: Uuid) -> Result<u64, DbErr> {
        Entity::find()
            .filter(
                Condition::all()
                    .add(Column::AppId.eq(app_id))
                    .add(Column::RevokedAt.is_null())
                    .add(Column::ExpiresAt.gt(chrono::Utc::now())),
            )
            .count(db)
            .await
    }
}

impl Model {
    pub fn is_active(&self) -> bool {
        self.revoked_at.is_none()
            && self.expires_at.with_timezone(&chrono::Utc) > chrono::Utc::now()
    }

    pub fn scopes_list(&self) -> Vec<String> {
        serde_json::from_value(self.scopes.clone()).unwrap_or_default()
    }
}
