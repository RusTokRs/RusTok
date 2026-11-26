use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, prelude::*, sea_query::Expr};

use super::_entities::oauth_authorization_codes::{self};
pub use super::_entities::oauth_authorization_codes::{ActiveModel, Column, Entity, Model};

impl Model {
    pub fn is_active(&self) -> bool {
        self.is_active_at(chrono::Utc::now())
    }

    /// A code cannot be used before its creation or at its exclusive expiry.
    pub fn is_active_at(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        self.used_at.is_none()
            && self.created_at.with_timezone(&chrono::Utc) <= now
            && self.expires_at.with_timezone(&chrono::Utc) > now
    }

    /// Decode the complete stored grant; never silently discard invalid entries.
    pub fn scopes_list(&self) -> Result<Vec<String>, serde_json::Error> {
        serde_json::from_value(self.scopes.clone())
    }
}

impl Entity {
    /// Atomically reserve an unused, unexpired authorization code with tenant and client scope.
    pub async fn find_by_hash_scoped(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        app_id: Uuid,
        code_hash: &str,
    ) -> Result<Option<Model>, DbErr> {
        let now = chrono::Utc::now();
        let reserved = Self::update_many()
            .col_expr(oauth_authorization_codes::Column::UsedAt, Expr::value(now))
            .filter(oauth_authorization_codes::Column::CodeHash.eq(code_hash))
            .filter(oauth_authorization_codes::Column::TenantId.eq(tenant_id))
            .filter(oauth_authorization_codes::Column::AppId.eq(app_id))
            .filter(oauth_authorization_codes::Column::UsedAt.is_null())
            .filter(oauth_authorization_codes::Column::CreatedAt.lte(now))
            .filter(oauth_authorization_codes::Column::ExpiresAt.gt(now))
            .exec(db)
            .await?;

        if reserved.rows_affected != 1 {
            return Ok(None);
        }

        let mut code = Self::find()
            .filter(oauth_authorization_codes::Column::CodeHash.eq(code_hash))
            .filter(oauth_authorization_codes::Column::TenantId.eq(tenant_id))
            .filter(oauth_authorization_codes::Column::AppId.eq(app_id))
            .one(db)
            .await?;
        if let Some(code) = code.as_mut() {
            code.used_at = None;
        }
        Ok(code)
    }
}
