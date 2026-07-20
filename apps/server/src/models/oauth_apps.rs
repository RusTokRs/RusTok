//! Business logic wrapper for OAuth apps

use rustok_api::Permission;
use sea_orm::{Condition, QueryFilter, entity::prelude::*};
use std::str::FromStr;
use uuid::Uuid;

pub use super::_entities::oauth_apps::{ActiveModel, Column, Entity, Model, Relation};

impl Entity {
    pub async fn find_active_by_client_id(
        db: &DatabaseConnection,
        client_id: Uuid,
    ) -> Result<Option<Model>, DbErr> {
        Entity::find()
            .filter(
                Condition::all()
                    .add(Column::ClientId.eq(client_id))
                    .add(Column::IsActive.eq(true))
                    .add(Column::RevokedAt.is_null()),
            )
            .one(db)
            .await
    }

    pub async fn find_by_tenant(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .all(db)
            .await
    }

    pub async fn find_active_by_tenant(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(
                Condition::all()
                    .add(Column::TenantId.eq(tenant_id))
                    .add(Column::IsActive.eq(true))
                    .add(Column::RevokedAt.is_null()),
            )
            .all(db)
            .await
    }
}

impl Model {
    pub fn is_active(&self) -> bool {
        self.is_active && self.revoked_at.is_none()
    }

    pub fn is_manual(&self) -> bool {
        !self.auto_created
    }

    pub fn managed_by_manifest(&self) -> bool {
        self.auto_created && self.manifest_ref.is_some()
    }

    /// Parse scopes from JSONB field
    pub fn scopes_list(&self) -> Vec<String> {
        serde_json::from_value(self.scopes.clone()).unwrap_or_default()
    }

    /// Parse grant_types from JSONB field
    pub fn grant_types_list(&self) -> Vec<String> {
        serde_json::from_value(self.grant_types.clone()).unwrap_or_default()
    }

    /// Parse granted_permissions from JSONB field
    pub fn granted_permissions_list(&self) -> Vec<String> {
        serde_json::from_value(self.granted_permissions.clone()).unwrap_or_default()
    }

    pub fn parsed_granted_permissions(&self) -> Result<Vec<Permission>, String> {
        self.granted_permissions_list()
            .into_iter()
            .map(|value| Permission::from_str(&value))
            .collect()
    }

    /// Parse redirect_uris from JSONB field
    pub fn redirect_uris_list(&self) -> Vec<String> {
        serde_json::from_value(self.redirect_uris.clone()).unwrap_or_default()
    }

    /// Check whether the app supports a grant type.
    ///
    /// Manifest-managed clients created before `refresh_token` became explicit
    /// historically received refresh tokens from `authorization_code`. Preserve
    /// that compatibility only for auto-created apps. Manual apps must declare
    /// `refresh_token` explicitly and therefore remain governed by strict grant
    /// configuration.
    pub fn supports_grant_type(&self, grant_type: &str) -> bool {
        let grants = self.grant_types_list();
        grants.iter().any(|value| value == grant_type)
            || (grant_type == "refresh_token"
                && self.auto_created
                && grants.iter().any(|value| value == "authorization_code"))
    }

    pub fn can_edit(&self) -> bool {
        self.is_manual() && matches!(self.app_type.as_str(), "third_party" | "mobile" | "service")
    }

    pub fn can_rotate_secret(&self) -> bool {
        if self.app_type == "embedded" {
            return false;
        }

        self.client_secret_hash.is_some()
    }

    pub fn can_revoke(&self) -> bool {
        self.is_manual() && matches!(self.app_type.as_str(), "third_party" | "mobile" | "service")
    }

    pub fn requires_user_consent(&self) -> bool {
        self.app_type == "third_party"
    }
}

#[cfg(test)]
mod tests {
    use super::Model;
    use sea_orm::prelude::DateTimeWithTimeZone;
    use uuid::Uuid;

    fn app(auto_created: bool, grants: serde_json::Value) -> Model {
        let now: DateTimeWithTimeZone = chrono::Utc::now().into();
        Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            name: "test".to_string(),
            slug: "test".to_string(),
            description: None,
            app_type: "third_party".to_string(),
            icon_url: None,
            client_id: Uuid::new_v4(),
            client_secret_hash: Some("hash".to_string()),
            redirect_uris: serde_json::json!([]),
            scopes: serde_json::json!([]),
            grant_types: grants,
            granted_permissions: serde_json::json!([]),
            manifest_ref: auto_created.then(|| "manifest".to_string()),
            auto_created,
            is_active: true,
            revoked_at: None,
            last_used_at: None,
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn manual_apps_require_explicit_refresh_grant() {
        let app = app(false, serde_json::json!(["authorization_code"]));
        assert!(!app.supports_grant_type("refresh_token"));
    }

    #[test]
    fn legacy_manifest_apps_preserve_authorization_code_refresh() {
        let app = app(true, serde_json::json!(["authorization_code"]));
        assert!(app.supports_grant_type("refresh_token"));
    }
}
