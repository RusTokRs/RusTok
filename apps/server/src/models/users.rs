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

    /// Exact recovery identity; callers separately apply active-status and
    /// purpose-specific requirements (including verification idempotency).
    pub(crate) fn matches_recovery_identity(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        email: &str,
        revision: i64,
    ) -> bool {
        self.tenant_id == tenant_id
            && self.id == user_id
            && self.email == email
            && self.recovery_revision == revision
            && revision >= 0
    }

    /// Fail closed for delegated OAuth credentials minted at or before the last
    /// password change. Callers pass the persisted authorization-code, refresh-
    /// token, or access-row creation time so subsecond ordering is preserved.
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
            recovery_revision: 0,
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }
}

impl ActiveModel {
    /// A trusted password replacement starts a new credential epoch and clears
    /// failures attributable to the old password. Do not use for login rehash.
    pub(crate) fn set_replacement_password(
        &mut self,
        password_hash: String,
        now: chrono::DateTime<chrono::Utc>,
    ) {
        use sea_orm::ActiveValue::Set;
        self.password_hash = Set(password_hash);
        self.password_changed_at = Set(Some(now.into()));
        self.failed_login_count = Set(0);
        self.locked_until = Set(None);
        self.updated_at = Set(now.into());
    }

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
            recovery_revision: sea_orm::ActiveValue::NotSet,
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
    use chrono::{Duration, TimeZone, Utc};

    #[test]
    fn password_replacement_clears_old_lockout_and_records_epoch() {
        use sea_orm::ActiveValue::Set;

        let now = Utc::now();
        let mut user = ActiveModel::new(Uuid::new_v4(), "user@example.test", "old-hash");
        user.failed_login_count = Set(4);
        user.locked_until = Set(Some((now + Duration::minutes(10)).into()));
        user.status = Set(UserStatus::Banned);

        user.set_replacement_password("new-hash".to_string(), now);

        assert_eq!(user.password_hash, Set("new-hash".to_string()));
        assert_eq!(user.password_changed_at, Set(Some(now.into())));
        assert_eq!(user.failed_login_count, Set(0));
        assert_eq!(user.locked_until, Set(None));
        assert_eq!(user.updated_at, Set(now.into()));
        assert_eq!(user.status, Set(UserStatus::Banned));
    }

    #[test]
    fn delegated_credential_epoch_rejects_tokens_issued_at_or_before_change() {
        let mut user = Model::default_service_user(Uuid::new_v4(), Uuid::new_v4());
        let changed_at = Utc.timestamp_opt(1_800_000_000, 900_000_000).unwrap();
        user.password_changed_at = Some(changed_at.into());

        assert!(user.rejects_delegated_token_issued_at(changed_at - Duration::seconds(1)));
        assert!(
            user.rejects_delegated_token_issued_at(
                Utc.timestamp_opt(1_800_000_000, 100_000_000).unwrap()
            ),
            "credential issued earlier in the same second must be rejected"
        );
        assert!(user.rejects_delegated_token_issued_at(changed_at));
        assert!(
            !user.rejects_delegated_token_issued_at(
                Utc.timestamp_opt(1_800_000_000, 950_000_000).unwrap()
            ),
            "credential issued later in the same second must be accepted"
        );
        assert!(!user.rejects_delegated_token_issued_at(changed_at + Duration::seconds(1)));
    }

    #[test]
    fn delegated_credential_epoch_is_absent_for_legacy_users() {
        let user = Model::default_service_user(Uuid::new_v4(), Uuid::new_v4());
        assert!(!user.rejects_delegated_token_issued_at(Utc::now()));
    }
}
