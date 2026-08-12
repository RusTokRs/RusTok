use std::collections::BTreeMap;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rustok_content::entities::node::ContentStatus;
use rustok_core::error::{ErrorKind, RichError};

use crate::dto::PageTranslationInput;
use crate::entities::{page, page_route_alias, page_route_publication, page_translation};
use crate::error::{PagesError, PagesResult};

use super::helpers::{normalize_locale, normalize_slug, status_to_storage, storage_to_status};

pub const PAGE_ROUTE_NOT_FOUND: &str = "PAGE_ROUTE_NOT_FOUND";
pub const PAGE_ROUTE_RESOLUTION_CONFLICT: &str = "PAGE_ROUTE_RESOLUTION_CONFLICT";

const ROUTE_DISPOSITION_REDIRECT: &str = "redirect";
const ROUTE_DISPOSITION_GONE: &str = "gone";
const PUBLISHED_SLUG_CHANGE_REASON: &str = "Published page slug changed";
const PAGE_DELETED_ROUTE_REASON: &str = "Page deleted";
const MAX_PAGE_ROUTE_ALIAS_REASON_LEN: usize = 500;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageRouteDisposition {
    Canonical,
    Redirect,
    Gone,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PageRouteDescriptor {
    pub page_id: Uuid,
    pub locale: String,
    pub slug: String,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PageRouteResolution {
    pub requested_locale: String,
    pub requested_slug: String,
    pub requested_page_id: Option<Uuid>,
    pub disposition: PageRouteDisposition,
    pub canonical: Option<PageRouteDescriptor>,
    pub alias_id: Option<Uuid>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CurrentPublishedRoute {
    page_id: Uuid,
    slug: String,
}

struct RedirectAliasRequest<'a> {
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &'a str,
    slug: &'a str,
    target_page_id: Uuid,
    target_locale: &'a str,
    reason: &'a str,
}

pub struct PageRouteService {
    db: DatabaseConnection,
}

impl PageRouteService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn canonical_descriptor(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
        locale: &str,
    ) -> PagesResult<PageRouteDescriptor> {
        let locale = normalize_locale(locale)?;
        let page = page::Entity::find_by_id(page_id)
            .filter(page::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(page_route_not_found)?;
        if storage_to_status(&page.status)? != ContentStatus::Published {
            return Err(page_route_not_found());
        }

        let translations = page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::PageId.eq(page_id))
            .filter(page_translation::Column::Locale.eq(&locale))
            .all(&self.db)
            .await?;
        let translation = match translations.as_slice() {
            [translation] => translation,
            [] => return Err(page_route_not_found()),
            _ => return Err(page_route_resolution_conflict()),
        };
        let slug = normalize_slug(&translation.slug)?;

        Ok(PageRouteDescriptor {
            page_id,
            path: page_route_path(&locale, &slug),
            locale,
            slug,
        })
    }

    pub async fn resolve(
        &self,
        tenant_id: Uuid,
        locale: &str,
        slug: &str,
    ) -> PagesResult<PageRouteResolution> {
        let locale = normalize_locale(locale)?;
        let slug = normalize_slug(slug)?;
        let current = load_current_published_routes(&self.db, tenant_id, &locale, &slug).await?;
        let aliases = page_route_alias::Entity::find()
            .filter(page_route_alias::Column::TenantId.eq(tenant_id))
            .filter(page_route_alias::Column::Locale.eq(&locale))
            .filter(page_route_alias::Column::Slug.eq(&slug))
            .order_by_asc(page_route_alias::Column::CreatedAt)
            .all(&self.db)
            .await?;

        match (current.as_slice(), aliases.as_slice()) {
            ([route], []) => {
                let canonical = self
                    .canonical_descriptor(tenant_id, route.page_id, &locale)
                    .await?;
                if canonical.slug != route.slug {
                    return Err(page_route_resolution_conflict());
                }
                Ok(PageRouteResolution {
                    requested_locale: locale,
                    requested_slug: slug,
                    requested_page_id: Some(route.page_id),
                    disposition: PageRouteDisposition::Canonical,
                    canonical: Some(canonical),
                    alias_id: None,
                })
            }
            ([], [alias]) => match alias.disposition.as_str() {
                ROUTE_DISPOSITION_GONE
                    if alias.target_page_id.is_none() && alias.target_locale.is_none() =>
                {
                    Ok(PageRouteResolution {
                        requested_locale: locale,
                        requested_slug: slug,
                        requested_page_id: Some(alias.page_id),
                        disposition: PageRouteDisposition::Gone,
                        canonical: None,
                        alias_id: Some(alias.id),
                    })
                }
                ROUTE_DISPOSITION_REDIRECT => {
                    let target_page_id = alias
                        .target_page_id
                        .ok_or_else(page_route_resolution_conflict)?;
                    let target_locale = alias
                        .target_locale
                        .as_deref()
                        .ok_or_else(page_route_resolution_conflict)?;
                    let target_exists = page::Entity::find_by_id(target_page_id)
                        .filter(page::Column::TenantId.eq(tenant_id))
                        .one(&self.db)
                        .await?
                        .is_some();
                    if !target_exists
                        && page_has_gone_tombstone(&self.db, tenant_id, target_page_id).await?
                    {
                        return Ok(PageRouteResolution {
                            requested_locale: locale,
                            requested_slug: slug,
                            requested_page_id: Some(alias.page_id),
                            disposition: PageRouteDisposition::Gone,
                            canonical: None,
                            alias_id: Some(alias.id),
                        });
                    }
                    let canonical = self
                        .canonical_descriptor(tenant_id, target_page_id, target_locale)
                        .await?;
                    Ok(PageRouteResolution {
                        requested_locale: locale,
                        requested_slug: slug,
                        requested_page_id: Some(alias.page_id),
                        disposition: PageRouteDisposition::Redirect,
                        canonical: Some(canonical),
                        alias_id: Some(alias.id),
                    })
                }
                _ => Err(page_route_resolution_conflict()),
            },
            ([], []) => Err(page_route_not_found()),
            _ => Err(page_route_resolution_conflict()),
        }
    }
}

pub(super) async fn ensure_route_alias_claim_available_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    locale: &str,
    slug: &str,
) -> PagesResult<()> {
    let locale = normalize_locale(locale)?;
    let slug = normalize_slug(slug)?;
    if page_route_alias::Entity::find()
        .filter(page_route_alias::Column::TenantId.eq(tenant_id))
        .filter(page_route_alias::Column::Locale.eq(&locale))
        .filter(page_route_alias::Column::Slug.eq(&slug))
        .one(txn)
        .await?
        .is_some()
    {
        return Err(PagesError::duplicate_slug(slug, locale));
    }
    Ok(())
}

pub(super) async fn record_published_route_snapshots_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    page_status: &str,
) -> PagesResult<u32> {
    if storage_to_status(page_status)? != ContentStatus::Published {
        return Ok(0);
    }

    let translations = page_translation::Entity::find()
        .filter(page_translation::Column::TenantId.eq(tenant_id))
        .filter(page_translation::Column::PageId.eq(page_id))
        .order_by_asc(page_translation::Column::Locale)
        .all(txn)
        .await?;
    let mut inserted = 0_u32;

    for translation in translations {
        let locale = normalize_locale(&translation.locale)?;
        let slug = normalize_slug(&translation.slug)?;
        let snapshots = page_route_publication::Entity::find()
            .filter(page_route_publication::Column::TenantId.eq(tenant_id))
            .filter(page_route_publication::Column::Locale.eq(&locale))
            .filter(page_route_publication::Column::Slug.eq(&slug))
            .all(txn)
            .await?;
        match snapshots.as_slice() {
            [] => {
                page_route_publication::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    page_id: Set(page_id),
                    locale: Set(locale),
                    slug: Set(slug),
                    recorded_at: Set(Utc::now().into()),
                }
                .insert(txn)
                .await?;
                inserted = inserted
                    .checked_add(1)
                    .ok_or_else(page_route_resolution_conflict)?;
            }
            [snapshot] if snapshot.page_id == page_id => {}
            _ => return Err(page_route_resolution_conflict()),
        }
    }

    Ok(inserted)
}

pub(super) async fn record_delete_route_tombstones_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
) -> PagesResult<u32> {
    let snapshots = page_route_publication::Entity::find()
        .filter(page_route_publication::Column::TenantId.eq(tenant_id))
        .filter(page_route_publication::Column::PageId.eq(page_id))
        .order_by_asc(page_route_publication::Column::RecordedAt)
        .all(txn)
        .await?;
    let reason = normalize_alias_reason(PAGE_DELETED_ROUTE_REASON)?;
    let mut inserted = 0_u32;

    for snapshot in snapshots {
        let aliases = page_route_alias::Entity::find()
            .filter(page_route_alias::Column::TenantId.eq(tenant_id))
            .filter(page_route_alias::Column::Locale.eq(&snapshot.locale))
            .filter(page_route_alias::Column::Slug.eq(&snapshot.slug))
            .all(txn)
            .await?;
        match aliases.as_slice() {
            [] => {
                record_gone_alias_in_tx(
                    txn,
                    tenant_id,
                    page_id,
                    &snapshot.locale,
                    &snapshot.slug,
                    &reason,
                )
                .await?;
                inserted = inserted
                    .checked_add(1)
                    .ok_or_else(page_route_resolution_conflict)?;
            }
            [alias]
                if alias.page_id == page_id
                    && alias.disposition == ROUTE_DISPOSITION_REDIRECT
                    && alias.target_page_id.is_some()
                    && alias.target_locale.is_some() =>
            {
                // Preserve immutable redirect history. Once the target page is
                // physically deleted, resolve() folds this route to Gone by the
                // target page's retained tombstone rather than rewriting history.
            }
            [alias]
                if alias.page_id == page_id
                    && alias.disposition == ROUTE_DISPOSITION_GONE
                    && alias.target_page_id.is_none()
                    && alias.target_locale.is_none()
                    && alias.reason == reason =>
            {
                // Exact replay is idempotent.
            }
            _ => return Err(page_route_resolution_conflict()),
        }
    }

    Ok(inserted)
}

pub(super) async fn record_published_slug_redirects_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    page_status: &str,
    translations: &[PageTranslationInput],
) -> PagesResult<()> {
    if storage_to_status(page_status)? != ContentStatus::Published {
        return Ok(());
    }

    let existing = page_translation::Entity::find()
        .filter(page_translation::Column::TenantId.eq(tenant_id))
        .filter(page_translation::Column::PageId.eq(page_id))
        .all(txn)
        .await?;
    let mut existing_by_locale = BTreeMap::new();
    for translation in existing {
        existing_by_locale.insert(normalize_locale(&translation.locale)?, translation);
    }

    for translation in translations {
        let locale = normalize_locale(&translation.locale)?;
        let new_slug = normalize_slug(
            translation
                .slug
                .as_deref()
                .unwrap_or(translation.title.as_str()),
        )?;
        let Some(existing) = existing_by_locale.get(&locale) else {
            continue;
        };
        let old_slug = normalize_slug(&existing.slug)?;
        if old_slug == new_slug {
            continue;
        }
        record_redirect_alias_in_tx(
            txn,
            RedirectAliasRequest {
                tenant_id,
                page_id,
                locale: &locale,
                slug: &old_slug,
                target_page_id: page_id,
                target_locale: &locale,
                reason: PUBLISHED_SLUG_CHANGE_REASON,
            },
        )
        .await?;
    }

    Ok(())
}

async fn record_redirect_alias_in_tx(
    txn: &DatabaseTransaction,
    request: RedirectAliasRequest<'_>,
) -> PagesResult<Uuid> {
    let locale = normalize_locale(request.locale)?;
    let slug = normalize_slug(request.slug)?;
    let target_locale = normalize_locale(request.target_locale)?;
    let reason = normalize_alias_reason(request.reason)?;
    let aliases = page_route_alias::Entity::find()
        .filter(page_route_alias::Column::TenantId.eq(request.tenant_id))
        .filter(page_route_alias::Column::Locale.eq(&locale))
        .filter(page_route_alias::Column::Slug.eq(&slug))
        .all(txn)
        .await?;

    match aliases.as_slice() {
        [] => {
            let alias_id = Uuid::new_v4();
            page_route_alias::ActiveModel {
                id: Set(alias_id),
                tenant_id: Set(request.tenant_id),
                page_id: Set(request.page_id),
                locale: Set(locale),
                slug: Set(slug),
                disposition: Set(ROUTE_DISPOSITION_REDIRECT.to_string()),
                target_page_id: Set(Some(request.target_page_id)),
                target_locale: Set(Some(target_locale)),
                reason: Set(reason),
                created_at: Set(Utc::now().into()),
            }
            .insert(txn)
            .await?;
            Ok(alias_id)
        }
        [alias]
            if alias.page_id == request.page_id
                && alias.disposition == ROUTE_DISPOSITION_REDIRECT
                && alias.target_page_id == Some(request.target_page_id)
                && alias.target_locale.as_deref() == Some(target_locale.as_str())
                && alias.reason == reason =>
        {
            Ok(alias.id)
        }
        _ => Err(page_route_resolution_conflict()),
    }
}

async fn record_gone_alias_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    slug: &str,
    reason: &str,
) -> PagesResult<Uuid> {
    let locale = normalize_locale(locale)?;
    let slug = normalize_slug(slug)?;
    let reason = normalize_alias_reason(reason)?;
    let alias_id = Uuid::new_v4();
    page_route_alias::ActiveModel {
        id: Set(alias_id),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        locale: Set(locale),
        slug: Set(slug),
        disposition: Set(ROUTE_DISPOSITION_GONE.to_string()),
        target_page_id: Set(None),
        target_locale: Set(None),
        reason: Set(reason),
        created_at: Set(Utc::now().into()),
    }
    .insert(txn)
    .await?;
    Ok(alias_id)
}

async fn page_has_gone_tombstone(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> PagesResult<bool> {
    Ok(page_route_alias::Entity::find()
        .filter(page_route_alias::Column::TenantId.eq(tenant_id))
        .filter(page_route_alias::Column::PageId.eq(page_id))
        .filter(page_route_alias::Column::Disposition.eq(ROUTE_DISPOSITION_GONE))
        .filter(page_route_alias::Column::TargetPageId.is_null())
        .filter(page_route_alias::Column::TargetLocale.is_null())
        .one(db)
        .await?
        .is_some())
}

async fn load_current_published_routes(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    locale: &str,
    slug: &str,
) -> PagesResult<Vec<CurrentPublishedRoute>> {
    let translations = page_translation::Entity::find()
        .filter(page_translation::Column::TenantId.eq(tenant_id))
        .filter(page_translation::Column::Locale.eq(locale))
        .filter(page_translation::Column::Slug.eq(slug))
        .all(db)
        .await?;
    let mut routes = Vec::new();
    for translation in translations {
        let page = page::Entity::find_by_id(translation.page_id)
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Status.eq(status_to_storage(&ContentStatus::Published)))
            .one(db)
            .await?;
        if page.is_some() {
            routes.push(CurrentPublishedRoute {
                page_id: translation.page_id,
                slug: normalize_slug(&translation.slug)?,
            });
        }
    }
    Ok(routes)
}

fn page_route_path(locale: &str, slug: &str) -> String {
    format!("/{locale}/modules/pages?slug={slug}")
}

fn normalize_alias_reason(reason: &str) -> PagesResult<String> {
    let reason = reason.trim();
    if reason.is_empty()
        || reason.chars().count() > MAX_PAGE_ROUTE_ALIAS_REASON_LEN
        || reason.chars().any(char::is_control)
    {
        return Err(PagesError::validation("Page route alias reason is invalid"));
    }
    Ok(reason.to_string())
}

fn page_route_not_found() -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(ErrorKind::NotFound, "Page route not found")
            .with_user_message("The requested page route does not exist")
            .with_error_code(PAGE_ROUTE_NOT_FOUND),
    ))
}

fn page_route_resolution_conflict() -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(
            ErrorKind::Conflict,
            "Page route ownership or alias history is ambiguous",
        )
        .with_user_message("The requested page route cannot be resolved safely")
        .with_error_code(PAGE_ROUTE_RESOLUTION_CONFLICT),
    ))
}
