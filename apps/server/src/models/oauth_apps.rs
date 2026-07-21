//! Business logic wrapper for OAuth apps

use rustok_api::Permission;
use sea_orm::{
    ActiveModelTrait, ActiveValue, Condition, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, QueryFilter, Set, TransactionTrait, entity::prelude::*,
};
use std::str::FromStr;
use uuid::Uuid;

use super::_entities::oauth_app_translations;
use super::_entities::oauth_apps::ActiveModel as DatabaseActiveModel;
pub use super::_entities::oauth_apps::{Column, Entity, Model, Relation};

const LEGACY_UNDETERMINED_LOCALE: &str = "und";

#[derive(Clone, Debug, Default)]
pub struct ActiveModel {
    pub id: ActiveValue<Uuid>,
    pub tenant_id: ActiveValue<Uuid>,
    pub name: ActiveValue<String>,
    pub slug: ActiveValue<String>,
    pub description: ActiveValue<Option<String>>,
    pub app_type: ActiveValue<String>,
    pub icon_url: ActiveValue<Option<String>>,
    pub client_id: ActiveValue<Uuid>,
    pub client_secret_hash: ActiveValue<Option<String>>,
    pub redirect_uris: ActiveValue<Json>,
    pub scopes: ActiveValue<Json>,
    pub grant_types: ActiveValue<Json>,
    pub granted_permissions: ActiveValue<Json>,
    pub manifest_ref: ActiveValue<Option<String>>,
    pub auto_created: ActiveValue<bool>,
    pub is_active: ActiveValue<bool>,
    pub revoked_at: ActiveValue<Option<DateTimeWithTimeZone>>,
    pub last_used_at: ActiveValue<Option<DateTimeWithTimeZone>>,
    pub metadata: ActiveValue<Json>,
    pub created_at: ActiveValue<DateTimeWithTimeZone>,
    pub updated_at: ActiveValue<DateTimeWithTimeZone>,
}

impl From<Model> for ActiveModel {
    fn from(model: Model) -> Self {
        Self {
            id: Set(model.id),
            tenant_id: Set(model.tenant_id),
            name: ActiveValue::NotSet,
            slug: Set(model.slug),
            description: ActiveValue::NotSet,
            app_type: Set(model.app_type),
            icon_url: Set(model.icon_url),
            client_id: Set(model.client_id),
            client_secret_hash: Set(model.client_secret_hash),
            redirect_uris: Set(model.redirect_uris),
            scopes: Set(model.scopes),
            grant_types: Set(model.grant_types),
            granted_permissions: Set(model.granted_permissions),
            manifest_ref: Set(model.manifest_ref),
            auto_created: Set(model.auto_created),
            is_active: Set(model.is_active),
            revoked_at: Set(model.revoked_at),
            last_used_at: Set(model.last_used_at),
            metadata: Set(model.metadata),
            created_at: Set(model.created_at),
            updated_at: Set(model.updated_at),
        }
    }
}

impl ActiveModel {
    pub async fn insert(self, db: &DatabaseConnection) -> Result<Model, DbErr> {
        let name = active_value(&self.name)
            .ok_or_else(|| DbErr::Custom("OAuth app name is required".to_string()))?;
        let description = active_value(&self.description).unwrap_or(None);
        let tenant_id = active_value(&self.tenant_id)
            .ok_or_else(|| DbErr::Custom("OAuth app tenant_id is required".to_string()))?;
        let app_id = active_value(&self.id)
            .ok_or_else(|| DbErr::Custom("OAuth app id is required".to_string()))?;

        let txn = db.begin().await?;
        let mut model = database_active(self).insert(&txn).await?;
        upsert_translation(
            &txn,
            tenant_id,
            app_id,
            LEGACY_UNDETERMINED_LOCALE,
            name.clone(),
            description.clone(),
        )
        .await?;
        txn.commit().await?;
        model.name = name;
        model.description = description;
        Ok(model)
    }

    pub async fn update<C>(self, db: &C) -> Result<Model, DbErr>
    where
        C: ConnectionTrait,
    {
        let name = active_value(&self.name);
        let description_changed = !matches!(self.description, ActiveValue::NotSet);
        let description = active_value(&self.description).unwrap_or(None);
        let tenant_id = active_value(&self.tenant_id)
            .ok_or_else(|| DbErr::Custom("OAuth app tenant_id is required".to_string()))?;
        let app_id = active_value(&self.id)
            .ok_or_else(|| DbErr::Custom("OAuth app id is required".to_string()))?;

        let mut model = database_active(self).update(db).await?;
        if name.is_some() || description_changed {
            let existing = oauth_app_translations::Entity::find()
                .filter(oauth_app_translations::Column::TenantId.eq(tenant_id))
                .filter(oauth_app_translations::Column::AppId.eq(app_id))
                .filter(oauth_app_translations::Column::Locale.eq(LEGACY_UNDETERMINED_LOCALE))
                .one(db)
                .await?;
            let resolved_name = name
                .clone()
                .or_else(|| existing.as_ref().map(|row| row.name.clone()))
                .ok_or_else(|| DbErr::Custom("OAuth app translation name is required".to_string()))?;
            let resolved_description = if description_changed {
                description.clone()
            } else {
                existing.and_then(|row| row.description)
            };
            upsert_translation(
                db,
                tenant_id,
                app_id,
                LEGACY_UNDETERMINED_LOCALE,
                resolved_name.clone(),
                resolved_description.clone(),
            )
            .await?;
            model.name = resolved_name;
            model.description = resolved_description;
        }
        Ok(model)
    }
}

fn database_active(model: ActiveModel) -> DatabaseActiveModel {
    let mut active = DatabaseActiveModel::default();
    active.id = model.id;
    active.tenant_id = model.tenant_id;
    active.slug = model.slug;
    active.app_type = model.app_type;
    active.icon_url = model.icon_url;
    active.client_id = model.client_id;
    active.client_secret_hash = model.client_secret_hash;
    active.redirect_uris = model.redirect_uris;
    active.scopes = model.scopes;
    active.grant_types = model.grant_types;
    active.granted_permissions = model.granted_permissions;
    active.manifest_ref = model.manifest_ref;
    active.auto_created = model.auto_created;
    active.is_active = model.is_active;
    active.revoked_at = model.revoked_at;
    active.last_used_at = model.last_used_at;
    active.metadata = model.metadata;
    active.created_at = model.created_at;
    active.updated_at = model.updated_at;
    active
}

fn active_value<T: Clone>(value: &ActiveValue<T>) -> Option<T> {
    match value {
        ActiveValue::Set(value) | ActiveValue::Unchanged(value) => Some(value.clone()),
        ActiveValue::NotSet => None,
    }
}

pub async fn upsert_translation<C>(
    db: &C,
    tenant_id: Uuid,
    app_id: Uuid,
    locale: &str,
    name: String,
    description: Option<String>,
) -> Result<oauth_app_translations::Model, DbErr>
where
    C: ConnectionTrait,
{
    let existing = oauth_app_translations::Entity::find()
        .filter(oauth_app_translations::Column::TenantId.eq(tenant_id))
        .filter(oauth_app_translations::Column::AppId.eq(app_id))
        .filter(oauth_app_translations::Column::Locale.eq(locale))
        .one(db)
        .await?;
    let now = chrono::Utc::now().into();
    match existing {
        Some(row) => {
            let mut active: oauth_app_translations::ActiveModel = row.into();
            active.name = Set(name);
            active.description = Set(description);
            active.updated_at = Set(now);
            active.update(db).await
        }
        None => {
            oauth_app_translations::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                app_id: Set(app_id),
                locale: Set(locale.to_string()),
                name: Set(name),
                description: Set(description),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(db)
            .await
        }
    }
}

pub async fn resolve_translation<C>(
    db: &C,
    tenant_id: Uuid,
    app_id: Uuid,
    locale: &str,
) -> Result<Option<oauth_app_translations::Model>, DbErr>
where
    C: ConnectionTrait,
{
    oauth_app_translations::Entity::find()
        .filter(oauth_app_translations::Column::TenantId.eq(tenant_id))
        .filter(oauth_app_translations::Column::AppId.eq(app_id))
        .filter(oauth_app_translations::Column::Locale.eq(locale))
        .one(db)
        .await
}

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

    pub fn scopes_list(&self) -> Vec<String> {
        serde_json::from_value(self.scopes.clone()).unwrap_or_default()
    }

    pub fn grant_types_list(&self) -> Vec<String> {
        serde_json::from_value(self.grant_types.clone()).unwrap_or_default()
    }

    pub fn granted_permissions_list(&self) -> Vec<String> {
        serde_json::from_value(self.granted_permissions.clone()).unwrap_or_default()
    }

    pub fn parsed_granted_permissions(&self) -> Result<Vec<Permission>, String> {
        self.granted_permissions_list()
            .into_iter()
            .map(|value| Permission::from_str(&value))
            .collect()
    }

    pub fn redirect_uris_list(&self) -> Vec<String> {
        serde_json::from_value(self.redirect_uris.clone()).unwrap_or_default()
    }

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
