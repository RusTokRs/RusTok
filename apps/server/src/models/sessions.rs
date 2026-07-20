use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, prelude::*, sea_query::Expr};

use rustok_core::generate_id;

use super::_entities::sessions::{self};
pub use super::_entities::sessions::{ActiveModel, Column, Entity, Model};

impl Model {
    pub fn is_active(&self) -> bool {
        self.revoked_at.is_none() && self.expires_at > Utc::now()
    }
}

impl ActiveModel {
    pub fn new(
        tenant_id: Uuid,
        user_id: Uuid,
        token_hash: String,
        expires_at: DateTime<Utc>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Self {
        Self {
            id: sea_orm::ActiveValue::Set(generate_id()),
            tenant_id: sea_orm::ActiveValue::Set(tenant_id),
            user_id: sea_orm::ActiveValue::Set(user_id),
            token_hash: sea_orm::ActiveValue::Set(token_hash),
            ip_address: sea_orm::ActiveValue::Set(ip_address),
            user_agent: sea_orm::ActiveValue::Set(user_agent),
            last_used_at: sea_orm::ActiveValue::NotSet,
            expires_at: sea_orm::ActiveValue::Set(expires_at.into()),
            revoked_at: sea_orm::ActiveValue::NotSet,
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
    }
}

impl Entity {
    /// Atomically reserve an active session refresh token for rotation.
    ///
    /// The persisted row is marked revoked before it is returned, so only one
    /// concurrent caller can consume a refresh token. The returned in-memory
    /// lease clears `revoked_at`; the lifecycle service then replaces the hash
    /// and writes the row back as the next active rotation. If processing fails
    /// after reservation, the old token remains revoked (fail closed).
    pub async fn find_by_token_hash(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        token_hash: &str,
    ) -> Result<Option<Model>, DbErr> {
        let now = Utc::now();
        let reserved = Self::update_many()
            .col_expr(sessions::Column::RevokedAt, Expr::value(now))
            .col_expr(sessions::Column::LastUsedAt, Expr::value(now))
            .col_expr(sessions::Column::UpdatedAt, Expr::value(now))
            .filter(sessions::Column::TenantId.eq(tenant_id))
            .filter(sessions::Column::TokenHash.eq(token_hash))
            .filter(sessions::Column::RevokedAt.is_null())
            .filter(sessions::Column::ExpiresAt.gt(now))
            .exec(db)
            .await?;

        if reserved.rows_affected != 1 {
            return Ok(None);
        }

        let mut session = Self::find()
            .filter(sessions::Column::TenantId.eq(tenant_id))
            .filter(sessions::Column::TokenHash.eq(token_hash))
            .one(db)
            .await?;
        if let Some(session) = session.as_mut() {
            session.revoked_at = None;
        }
        Ok(session)
    }
}
