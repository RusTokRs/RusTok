//! Pages-owned layout catalog. This is authoring state, not a public artifact.
//! `default` remains the virtual empty layout until legacy template labels
//! have been explicitly audited and publish-time composition is enabled.

use std::collections::BTreeSet;

use chrono::Utc;
use rustok_api::{Action, Resource, TenantLocale};
use rustok_core::{PermissionScope, SecurityContext};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, TransactionTrait, sea_query::Expr};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{page, page_template, site_symbol};
use crate::error::{PagesError, PagesResult};

pub const MAX_TEMPLATE_SECTIONS: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PageTemplateRecord {
    pub key: String,
    pub locale: String,
    pub header_symbol_ids: Vec<String>,
    pub footer_symbol_ids: Vec<String>,
    pub revision: i64,
}

impl TryFrom<page_template::Model> for PageTemplateRecord {
    type Error = PagesError;

    fn try_from(row: page_template::Model) -> PagesResult<Self> {
        Ok(Self {
            key: row.template_key,
            locale: row.locale,
            header_symbol_ids: serde_json::from_value(row.header_symbol_ids)
                .map_err(|error| PagesError::validation(format!("corrupt template header: {error}")))?,
            footer_symbol_ids: serde_json::from_value(row.footer_symbol_ids)
                .map_err(|error| PagesError::validation(format!("corrupt template footer: {error}")))?,
            revision: row.revision,
        })
    }
}

#[derive(Clone)]
pub struct PageTemplateService {
    db: DatabaseConnection,
}

impl PageTemplateService {
    pub fn new(db: DatabaseConnection) -> Self { Self { db } }

    /// List exact-locale templates for a tenant. No fallback or implicit `default` row.
    pub async fn list(&self, tenant_id: Uuid, security: &SecurityContext, locale: &str) -> PagesResult<Vec<PageTemplateRecord>> {
        require_all(security, Action::Read)?;
        let locale = normalize_locale(locale)?;
        page_template::Entity::find()
            .filter(page_template::Column::TenantId.eq(tenant_id))
            .filter(page_template::Column::Locale.eq(locale))
            .order_by_asc(page_template::Column::TemplateKey)
            .all(&self.db).await?
            .into_iter().map(TryInto::try_into).collect()
    }

    /// Create with `expected_revision = None`; update with the exact observed revision.
    /// References are checked under the same catalog writer lock as symbol edits.
    pub async fn save(&self, tenant_id: Uuid, security: &SecurityContext, key: &str, locale: &str, header: Vec<String>, footer: Vec<String>, expected_revision: Option<i64>) -> PagesResult<PageTemplateRecord> {
        require_all(security, Action::Manage)?;
        let key = normalize_key(key)?;
        let locale = normalize_locale(locale)?;
        let ids = validate_slots(&header, &footer)?;
        let tx = self.db.begin().await?;
        // Symbol deletion and template insertion cannot race on Postgres.
        crate::services::page::lock_site_symbol_catalog_in_tx(&tx, tenant_id, &locale).await?;
        let present = site_symbol::Entity::find()
            .filter(site_symbol::Column::TenantId.eq(tenant_id))
            .filter(site_symbol::Column::Locale.eq(&locale))
            .filter(site_symbol::Column::SymbolId.is_in(ids.iter().cloned()))
            .all(&tx).await?
            .into_iter().map(|row| row.symbol_id).collect::<BTreeSet<_>>();
        if ids != present {
            return Err(PagesError::validation("Template references a missing site symbol in this tenant/locale"));
        }
        let existing = page_template::Entity::find_by_id((tenant_id, locale.clone(), key.clone())).one(&tx).await?;
        let now = Utc::now();
        let header_value = serde_json::json!(header.clone());
        let footer_value = serde_json::json!(footer.clone());
        let revision = match existing {
            Some(row) => {
                if expected_revision != Some(row.revision) {
                    return Err(PagesError::translation_conflict("Template revision changed; reload before saving"));
                }
                let next = row.revision.checked_add(1).ok_or_else(|| PagesError::validation("Template revision exhausted"))?;
                let updated = page_template::Entity::update_many()
                    .col_expr(page_template::Column::HeaderSymbolIds, Expr::value(header_value.clone()))
                    .col_expr(page_template::Column::FooterSymbolIds, Expr::value(footer_value.clone()))
                    .col_expr(page_template::Column::Revision, Expr::value(next))
                    .col_expr(page_template::Column::UpdatedAt, Expr::value(now))
                    .filter(page_template::Column::TenantId.eq(tenant_id))
                    .filter(page_template::Column::Locale.eq(&locale))
                    .filter(page_template::Column::TemplateKey.eq(&key))
                    .filter(page_template::Column::Revision.eq(row.revision))
                    .exec(&tx).await?;
                if updated.rows_affected != 1 {
                    return Err(PagesError::translation_conflict("Template changed during save"));
                }
                next
            }
            None => {
                if expected_revision.is_some() {
                    return Err(PagesError::translation_conflict("Template does not exist at the expected revision"));
                }
                page_template::ActiveModel {
                    tenant_id: Set(tenant_id), locale: Set(locale.clone()), template_key: Set(key.clone()),
                    header_symbol_ids: Set(header_value), footer_symbol_ids: Set(footer_value),
                    revision: Set(1), created_at: Set(now.into()), updated_at: Set(now.into()),
                }.insert(&tx).await?;
                1
            }
        };
        tx.commit().await?;
        Ok(PageTemplateRecord { key, locale, header_symbol_ids: header, footer_symbol_ids: footer, revision })
    }

    /// Do not remove a template while *any* page still carries its key.
    pub async fn delete(&self, tenant_id: Uuid, security: &SecurityContext, key: &str, locale: &str, expected_revision: i64) -> PagesResult<()> {
        require_all(security, Action::Manage)?;
        let key = normalize_key(key)?;
        let locale = normalize_locale(locale)?;
        let tx = self.db.begin().await?;
        crate::services::page::lock_site_symbol_catalog_in_tx(&tx, tenant_id, &locale).await?;
        let used = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Template.eq(&key))
            .one(&tx).await?.is_some();
        if used { return Err(PagesError::validation("Cannot delete a template used by a page")); }
        let deleted = page_template::Entity::delete_many()
            .filter(page_template::Column::TenantId.eq(tenant_id))
            .filter(page_template::Column::Locale.eq(&locale))
            .filter(page_template::Column::TemplateKey.eq(&key))
            .filter(page_template::Column::Revision.eq(expected_revision))
            .exec(&tx).await?;
        if deleted.rows_affected != 1 {
            return Err(PagesError::translation_conflict("Template revision changed or template not found"));
        }
        tx.commit().await?;
        Ok(())
    }
}

fn require_all(security: &SecurityContext, action: Action) -> PagesResult<()> {
    if security.get_scope(Resource::Pages, action) != PermissionScope::All {
        return Err(PagesError::forbidden("Site template catalog requires tenant-wide Pages permission"));
    }
    Ok(())
}

fn normalize_locale(value: &str) -> PagesResult<String> {
    TenantLocale::new(value).map(TenantLocale::into_inner)
        .map_err(|_| PagesError::validation("Invalid template locale"))
}

fn normalize_key(value: &str) -> PagesResult<String> {
    if value != value.trim() || value == "default" || value.len() > 128 {
        return Err(PagesError::validation("Invalid template key (default is reserved)"));
    }
    fly::validate_identifier(value)
        .map_err(|reason| PagesError::validation(format!("Invalid template key: {reason}")))?;
    Ok(value.to_string())
}

fn validate_slots(header: &[String], footer: &[String]) -> PagesResult<BTreeSet<String>> {
    if header.len() > MAX_TEMPLATE_SECTIONS || footer.len() > MAX_TEMPLATE_SECTIONS {
        return Err(PagesError::validation("Template exceeds the section limit"));
    }
    let mut ids = BTreeSet::new();
    for id in header.iter().chain(footer) {
        fly::validate_identifier(id).map_err(PagesError::validation)?;
        if !ids.insert(id.clone()) {
            return Err(PagesError::validation("Duplicate symbol in template layout"));
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_sections_fail_closed() {
        assert!(normalize_key("default").is_err());
        assert!(normalize_key(" bad ").is_err());
        assert!(normalize_key("marketing").is_ok());
        assert!(validate_slots(&["header".into()], &["header".into()]).is_err());
        assert!(validate_slots(&["header".into()], &["footer".into()]).is_ok());
        assert!(validate_slots(&["evil/id".into()], &[]).is_err());
    }
}
