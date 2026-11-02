use sea_orm::prelude::*;

use rustok_core::field_schema::HasCustomFields;
use rustok_core::{UserStatus, generate_id};

use super::_entities::users::{self};
pub use super::_entities::users::{ActiveModel, Column, Entity, Model};

impl Model {
    pub fn is_active(&self) -> bool {
        self.status == UserStatus::Active
    }

    pub fn is_email_verified(&self) -> bool {
        self.email_verified_at.is_some()
    }

    /// Fail closed for delegated OAuth tokens minted at or before the last
    /// password change. JWT `iat` has second granularity, so equality is
    /// rejected to avoid accepting a token issued earlier in the same second.
    pub fn rejects_delegated_token_issued_at(
        &self,
        issued_at: chrono::DateTime<chrono::Utc>,
    ) -> bool {
        self.password_changed_at
            .map(|changed_at| issued_at <= changed_at.with_timezone(&chrono::Utc))
            .unwrap_or(false)
    }

    /// Create a minimal in-memory user model for OAuth2 service tokens.
    /// Used when the JWT `sub` is an app_id, not a real user_id.
    pub fn default_service_user(app_id: Uuid, tenant_id: Uuid) -> Self {
        Self {
            id: app_id,
            tenant_id,
            email: format!("service+{}@oauth.internal", app_id),
            password_hash: String::new(),
            name: Some("OAuth Service".to_string()),
            status: UserStatus::Active,
            email_verified_at: None,
            last_login_at: None,
            failed_login_count: 0,
            locked_until: None,
            password_changed_at: None,
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }
}

impl ActiveModel {
    pub fn new(tenant_id: Uuid, email: &str, password_hash: &str) -> Self {
        Self {
            id: sea_orm::ActiveValue::Set(generate_id()),
            tenant_id: sea_orm::ActiveValue::Set(tenant_id),
            email: sea_orm::ActiveValue::Set(email.to_lowercase()),
            password_hash: sea_orm::ActiveValue::Set(password_hash.to_string()),
            name: sea_orm::ActiveValue::NotSet,
            status: sea_orm::ActiveValue::Set(UserStatus::Active),
            email_verified_at: sea_orm::ActiveValue::NotSet,
            last_login_at: sea_orm::ActiveValue::NotSet,
            failed_login_count: sea_orm::ActiveValue::Set(0),
            locked_until: sea_orm::ActiveValue::Set(None),
            password_changed_at: sea_orm::ActiveValue::NotSet,
            metadata: sea_orm::ActiveValue::Set(serde_json::json!({})),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
    }
}

impl HasCustomFields for Model {
    fn entity_type() -> &'static str {
        "user"
    }

    fn metadata(&self) -> &serde_json::Value {
        &self.metadata
    }

    fn set_metadata(&mut self, value: serde_json::Value) {
        self.metadata = value;
    }
}

impl Entity {
    pub async fn find_by_email(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        email: &str,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(users::Column::TenantId.eq(tenant_id))
            .filter(users::Column::Email.eq(email.to_lowercase()))
            .one(db)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn delegated_credential_epoch_rejects_tokens_issued_at_or_before_change() {
        let mut user = Model::default_service_user(Uuid::new_v4(), Uuid::new_v4());
        let changed_at = Utc::now();
        user.password_changed_at = Some(changed_at.into());

        assert!(user.rejects_delegated_token_issued_at(changed_at - Duration::seconds(1)));
        assert!(user.rejects_delegated_token_issued_at(changed_at));
        assert!(!user.rejects_delegated_token_issued_at(changed_at + Duration::seconds(1)));
    }

    #[test]
    fn delegated_credential_epoch_is_absent_for_legacy_users() {
        let user = Model::default_service_user(Uuid::new_v4(), Uuid::new_v4());
        assert!(!user.rejects_delegated_token_issued_at(Utc::now()));
    }
}
