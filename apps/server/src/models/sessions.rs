use chrono::{DateTime, Duration, Utc};

use rustok_core::generate_id;

pub use super::_entities::sessions::{ActiveModel, Column, Entity, Model};

/// Hard cap on a refresh lineage: no amount of sliding rotation extends
/// a session beyond this many days from the original login.
pub const SESSION_ABSOLUTE_LIFETIME_DAYS: i64 = 90;

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
            previous_token_hash: sea_orm::ActiveValue::NotSet,
            previous_token_revoked_at: sea_orm::ActiveValue::NotSet,
            absolute_expires_at: sea_orm::ActiveValue::Set(Some(
                (Utc::now() + Duration::days(SESSION_ABSOLUTE_LIFETIME_DAYS)).into(),
            )),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
    }
}

