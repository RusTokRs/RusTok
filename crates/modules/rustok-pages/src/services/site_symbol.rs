//! Site symbols catalog service.
//!
//! Authoring state for tenant-wide, localized shared symbols (headers, footers, callouts).

use chrono::Utc;
use rustok_api::{Action, Resource, TenantLocale};
use rustok_core::{PermissionScope, SecurityContext};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::site_symbol;
use crate::error::{PagesError, PagesResult};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SiteSymbolRecord {
    pub symbol_id: String,
    pub locale: String,
    pub name: Option<String>,
    pub content: serde_json::Value,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

impl TryFrom<site_symbol::Model> for SiteSymbolRecord {
    type Error = PagesError;

    fn try_from(row: site_symbol::Model) -> PagesResult<Self> {
        Ok(Self {
            symbol_id: row.symbol_id,
            locale: row.locale,
            name: row.name,
            content: row.content,
            created_at: row.created_at.into(),
            updated_at: row.updated_at.into(),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveSiteSymbolInput {
    pub symbol_id: String,
    pub locale: String,
    pub name: Option<String>,
    pub content: serde_json::Value,
}

#[derive(Clone)]
pub struct SiteSymbolService {
    db: DatabaseConnection,
}

impl SiteSymbolService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// List exact-locale symbols for a tenant, ordered alphabetically by symbol ID.
    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        locale: &str,
    ) -> PagesResult<Vec<SiteSymbolRecord>> {
        require_all(security, Action::Manage)?;
        let locale = normalize_locale(locale)?;
        site_symbol::Entity::find()
            .filter(site_symbol::Column::TenantId.eq(tenant_id))
            .filter(site_symbol::Column::Locale.eq(locale))
            .order_by_asc(site_symbol::Column::SymbolId)
            .all(&self.db)
            .await?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    /// Retrieve a single site symbol by identity `(tenant, locale, symbol_id)`.
    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        symbol_id: &str,
        locale: &str,
    ) -> PagesResult<Option<SiteSymbolRecord>> {
        require_all(security, Action::Manage)?;
        let locale = normalize_locale(locale)?;
        let symbol_id = normalize_id(symbol_id)?;
        site_symbol::Entity::find_by_id((tenant_id, locale, symbol_id))
            .one(&self.db)
            .await?
            .map(TryInto::try_into)
            .transpose()
    }

    /// Create or update a site symbol. Content is validated as a well-formed Fly `SymbolDescriptor`.
    pub async fn save(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        input: SaveSiteSymbolInput,
    ) -> PagesResult<SiteSymbolRecord> {
        require_all(security, Action::Manage)?;
        let locale = normalize_locale(&input.locale)?;
        let symbol_id = normalize_id(&input.symbol_id)?;

        let mut content = input.content;
        if let Some(obj) = content.as_object_mut() {
            obj.insert(
                "id".to_string(),
                serde_json::Value::String(symbol_id.clone()),
            );
            if let Some(name) = &input.name {
                obj.insert("name".to_string(), serde_json::Value::String(name.clone()));
            }
        }

        let descriptor = fly::SymbolDescriptor::from_value(&content).ok_or_else(|| {
            PagesError::validation(
                "Site symbol content must be a valid symbol descriptor with non-empty id",
            )
        })?;

        let encoded_bytes = serde_json::to_vec(&content)
            .map_err(|e| PagesError::validation(format!("Cannot encode symbol content: {e}")))?;
        if encoded_bytes.len() > fly::MAX_SYMBOL_DEFINITION_BYTES {
            return Err(PagesError::validation(format!(
                "Site symbol content exceeds maximum allowed size of {} bytes",
                fly::MAX_SYMBOL_DEFINITION_BYTES
            )));
        }

        let name = input.name.or(descriptor.name);
        let now = Utc::now();

        let tx = self.db.begin().await?;
        crate::services::page::lock_site_symbol_catalog_in_tx(&tx, tenant_id, &locale).await?;

        let existing =
            site_symbol::Entity::find_by_id((tenant_id, locale.clone(), symbol_id.clone()))
                .one(&tx)
                .await?;

        let record = match existing {
            Some(row) => {
                let mut active: site_symbol::ActiveModel = row.into();
                active.name = Set(name.clone());
                active.content = Set(content.clone());
                active.updated_at = Set(now.into());
                let updated = active.update(&tx).await?;
                SiteSymbolRecord::try_from(updated)?
            }
            None => {
                let active = site_symbol::ActiveModel {
                    tenant_id: Set(tenant_id),
                    locale: Set(locale.clone()),
                    symbol_id: Set(symbol_id.clone()),
                    name: Set(name.clone()),
                    content: Set(content.clone()),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                };
                let inserted = active.insert(&tx).await?;
                SiteSymbolRecord::try_from(inserted)?
            }
        };

        tx.commit().await?;
        Ok(record)
    }

    /// Delete a site symbol only when it is not referenced by any layout template or active page.
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        symbol_id: &str,
        locale: &str,
    ) -> PagesResult<()> {
        require_all(security, Action::Manage)?;
        let locale = normalize_locale(locale)?;
        let symbol_id = normalize_id(symbol_id)?;

        let tx = self.db.begin().await?;
        crate::services::page::lock_site_symbol_catalog_in_tx(&tx, tenant_id, &locale).await?;

        let existing =
            site_symbol::Entity::find_by_id((tenant_id, locale.clone(), symbol_id.clone()))
                .one(&tx)
                .await?;
        if existing.is_none() {
            return Err(PagesError::validation("Site symbol not found"));
        }

        let template_refs =
            crate::services::page::template_symbol_ids(&tx, tenant_id, &locale).await?;
        if template_refs.contains(&symbol_id) {
            return Err(PagesError::validation(format!(
                "Cannot delete site symbol `{symbol_id}` because it is referenced by a page template"
            )));
        }

        if crate::services::page::symbol_used_by_any_page(&tx, tenant_id, &locale, &symbol_id)
            .await?
        {
            return Err(PagesError::validation(format!(
                "Cannot delete site symbol `{symbol_id}` because it is referenced by a page"
            )));
        }

        let deleted = site_symbol::Entity::delete_many()
            .filter(site_symbol::Column::TenantId.eq(tenant_id))
            .filter(site_symbol::Column::Locale.eq(&locale))
            .filter(site_symbol::Column::SymbolId.eq(&symbol_id))
            .exec(&tx)
            .await?;

        if deleted.rows_affected != 1 {
            return Err(PagesError::validation(
                "Site symbol not found during deletion",
            ));
        }

        tx.commit().await?;
        Ok(())
    }
}

fn require_all(security: &SecurityContext, action: Action) -> PagesResult<()> {
    if security.get_scope(Resource::Pages, action) != PermissionScope::All {
        return Err(PagesError::forbidden(
            "Site symbol authoring requires tenant-wide Pages permission",
        ));
    }
    Ok(())
}

fn normalize_locale(value: &str) -> PagesResult<String> {
    TenantLocale::new(value)
        .map(TenantLocale::into_inner)
        .map_err(|_| PagesError::validation("Invalid symbol locale"))
}

fn normalize_id(value: &str) -> PagesResult<String> {
    let trimmed = value.trim();
    if trimmed != value || trimmed.is_empty() || trimmed.len() > 128 {
        return Err(PagesError::validation("Invalid site symbol identifier"));
    }
    fly::validate_identifier(trimmed).map_err(|reason| {
        PagesError::validation(format!("Invalid site symbol identifier: {reason}"))
    })?;
    Ok(trimmed.to_string())
}
