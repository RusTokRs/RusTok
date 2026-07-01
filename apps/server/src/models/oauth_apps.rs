//! Business logic wrapper for OAuth apps

use rustok_api::Permission;
use sea_orm::{entity::prelude::*, Condition, QueryFilter};
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

    /// Check if the app supports a specific grant type
    pub fn supports_grant_type(&self, grant_type: &str) -> bool {
        self.grant_types_list().iter().any(|gt| gt == grant_type)
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
