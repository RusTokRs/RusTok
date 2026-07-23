//! Business logic wrapper for OAuth apps

use rustok_api::{Permission, normalize_locale_tag};
use sea_orm::sea_query::{Alias, OnConflict, Query};
use sea_orm::{
    ActiveModelTrait, ActiveValue, Condition, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, DbErr, EntityTrait, QueryFilter, Set, TransactionTrait,
    entity::prelude::*,
};
use std::{future::Future, str::FromStr};
use uuid::Uuid;

use super::_entities::oauth_apps::ActiveModel as DatabaseActiveModel;
pub use super::_entities::oauth_apps::{Column, Entity, Model, Relation};
use super::_entities::{oauth_app_translations, tenants};

const LEGACY_UNDETERMINED_LOCALE: &str = "und";
const MANIFEST_GENERATED_COPY_LOCALE: &str = "en";

tokio::task_local! {
    static OAUTH_RUNTIME_COPY_LOCALE: String;
}

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

#[async_trait::async_trait]
pub trait OAuthAppUpdateConnection: ConnectionTrait {
    async fn update_oauth_app_model(&self, model: ActiveModel) -> Result<Model, DbErr>;
}

#[async_trait::async_trait]
impl OAuthAppUpdateConnection for DatabaseConnection {
    async fn update_oauth_app_model(&self, model: ActiveModel) -> Result<Model, DbErr> {
        let transaction = self.begin().await?;
        let model = model.update_in_transaction(&transaction).await?;
        transaction.commit().await?;
        Ok(model)
    }
}

#[async_trait::async_trait]
impl OAuthAppUpdateConnection for DatabaseTransaction {
    async fn update_oauth_app_model(&self, model: ActiveModel) -> Result<Model, DbErr> {
        model.update_in_transaction(self).await
    }
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
        let auto_created = active_value(&self.auto_created).unwrap_or(false);
        let locale = copy_write_locale(auto_created)?;

        let transaction = db.begin().await?;
        let mut model = database_active(self).insert(&transaction).await?;
        upsert_translation(
            &transaction,
            tenant_id,
            app_id,
            locale.as_str(),
            name.clone(),
            description.clone(),
        )
        .await?;
        transaction.commit().await?;
        model.name = name;
        model.description = description;
        Ok(model)
    }

    pub async fn update<C>(self, db: &C) -> Result<Model, DbErr>
    where
        C: OAuthAppUpdateConnection,
    {
        db.update_oauth_app_model(self).await
    }

    async fn update_in_transaction<C>(self, db: &C) -> Result<Model, DbErr>
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
        let auto_created = active_value(&self.auto_created).unwrap_or(false);

        let localized_update = if name.is_some() || description_changed {
            let locale = copy_write_locale(auto_created)?;
            let existing = resolve_translation(db, tenant_id, app_id, locale.as_str()).await?;
            let resolved_name = name
                .clone()
                .or_else(|| existing.as_ref().map(|row| row.name.clone()))
                .ok_or_else(|| {
                    DbErr::Custom(format!(
                        "OAuth app translation name is required for locale `{locale}`"
                    ))
                })?;
            let resolved_description = if description_changed {
                description.clone()
            } else {
                existing.and_then(|row| row.description)
            };
            Some((locale, resolved_name, resolved_description))
        } else {
            None
        };

        let mut model = database_active(self).update(db).await?;
        if let Some((locale, resolved_name, resolved_description)) = localized_update {
            upsert_translation(
                db,
                tenant_id,
                app_id,
                locale.as_str(),
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
    let mut active = <DatabaseActiveModel as Default>::default();
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

fn active_value<T>(value: &ActiveValue<T>) -> Option<T>
where
    T: Clone + Into<sea_orm::Value>,
{
    match value {
        ActiveValue::Set(value) | ActiveValue::Unchanged(value) => Some(value.clone()),
        ActiveValue::NotSet => None,
    }
}

pub fn normalize_runtime_copy_locale(value: &str) -> Result<String, DbErr> {
    let locale = normalize_locale_tag(value).ok_or_else(|| {
        DbErr::Custom(
            "OAuth app locale must be a normalized BCP47-like tag with at most 32 bytes"
                .to_string(),
        )
    })?;
    if locale == LEGACY_UNDETERMINED_LOCALE {
        return Err(DbErr::Custom(
            "OAuth runtime copy cannot use storage-only locale `und`".to_string(),
        ));
    }
    Ok(locale)
}

pub async fn scope_runtime_copy_locale<F>(locale: String, future: F) -> F::Output
where
    F: Future,
{
    OAUTH_RUNTIME_COPY_LOCALE.scope(locale, future).await
}

fn copy_write_locale(auto_created: bool) -> Result<String, DbErr> {
    if auto_created {
        return Ok(MANIFEST_GENERATED_COPY_LOCALE.to_string());
    }
    OAUTH_RUNTIME_COPY_LOCALE
        .try_with(Clone::clone)
        .map_err(|_| {
            DbErr::Custom(
                "manual OAuth app display writes require a request-scoped effective locale"
                    .to_string(),
            )
        })
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
    let locale = normalize_runtime_copy_locale(locale)?;
    let now: DateTimeWithTimeZone = chrono::Utc::now().into();
    let mut insert = Query::insert();
    insert
        .into_table(Alias::new("oauth_app_translations"))
        .columns([
            Alias::new("id"),
            Alias::new("tenant_id"),
            Alias::new("app_id"),
            Alias::new("locale"),
            Alias::new("name"),
            Alias::new("description"),
            Alias::new("created_at"),
            Alias::new("updated_at"),
        ])
        .values_panic([
            Uuid::new_v4().into(),
            tenant_id.into(),
            app_id.into(),
            locale.clone().into(),
            name.into(),
            description.into(),
            now.into(),
            now.into(),
        ])
        .on_conflict(
            OnConflict::columns([
                Alias::new("tenant_id"),
                Alias::new("app_id"),
                Alias::new("locale"),
            ])
            .update_column(Alias::new("name"))
            .update_column(Alias::new("description"))
            .update_column(Alias::new("updated_at"))
            .to_owned(),
        );
    db.execute(db.get_database_backend().build(&insert)).await?;

    resolve_translation(db, tenant_id, app_id, locale.as_str())
        .await?
        .ok_or_else(|| {
            DbErr::Custom(format!(
                "OAuth app translation missing after upsert: app {app_id}, locale `{locale}`"
            ))
        })
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
    let locale = normalize_runtime_copy_locale(locale)?;
    oauth_app_translations::Entity::find()
        .filter(oauth_app_translations::Column::TenantId.eq(tenant_id))
        .filter(oauth_app_translations::Column::AppId.eq(app_id))
        .filter(oauth_app_translations::Column::Locale.eq(locale))
        .one(db)
        .await
}

pub async fn hydrate_exact_translation<C>(
    db: &C,
    mut model: Model,
    locale: &str,
) -> Result<Model, DbErr>
where
    C: ConnectionTrait,
{
    let locale = normalize_runtime_copy_locale(locale)?;
    let translation = resolve_translation(db, model.tenant_id, model.id, locale.as_str())
        .await?
        .ok_or_else(|| {
            DbErr::Custom(format!(
                "OAuth app translation missing: app {}, locale `{locale}`",
                model.id
            ))
        })?;
    model.name = translation.name;
    model.description = translation.description;
    Ok(model)
}

async fn hydrate_tenant_default_or_identifier(
    db: &DatabaseConnection,
    mut model: Model,
) -> Result<Model, DbErr> {
    let tenant = tenants::Entity::find_by_id(db, model.tenant_id).await?;
    if let Some(locale) = tenant
        .as_ref()
        .and_then(|tenant| normalize_locale_tag(tenant.default_locale.as_str()))
        .filter(|locale| locale != LEGACY_UNDETERMINED_LOCALE)
    {
        if let Some(translation) =
            resolve_translation(db, model.tenant_id, model.id, &locale).await?
        {
            model.name = translation.name;
            model.description = translation.description;
            return Ok(model);
        }
    }

    // OAuth security identity must remain usable even when presentation copy for the
    // tenant policy locale is missing. The stable slug is an identifier, not a
    // localized fallback row, and `und` is never returned at runtime.
    model.name = model.slug.clone();
    model.description = None;
    Ok(model)
}

impl Entity {
    pub async fn find_active_by_client_id(
        db: &DatabaseConnection,
        client_id: Uuid,
    ) -> Result<Option<Model>, DbErr> {
        let model = Entity::find()
            .filter(
                Condition::all()
                    .add(Column::ClientId.eq(client_id))
                    .add(Column::IsActive.eq(true))
                    .add(Column::RevokedAt.is_null()),
            )
            .one(db)
            .await?;
        match model {
            Some(model) => Ok(Some(hydrate_tenant_default_or_identifier(db, model).await?)),
            None => Ok(None),
        }
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
