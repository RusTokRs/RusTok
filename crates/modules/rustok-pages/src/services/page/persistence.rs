use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseTransaction,
    DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect, prelude::DateTimeWithTimeZone,
};
use uuid::Uuid;

use crate::dto::{PageBodyRevisionSource, PageTranslationInput};
use crate::entities::{
    page, page_body, page_body_draft, page_body_revision, page_channel_visibility,
    page_route_publication, page_translation,
};
use crate::error::{PagesError, PagesResult};

use super::helpers::{
    merge_working_bodies, next_page_translation_revision, normalize_locale, normalize_slug,
};
use super::route::ensure_route_alias_claim_available_in_tx;
use super::{PageService, PreparedPageBody, WorkingBody};

/// Newest body revisions kept per (page, locale) in the append-only journal.
pub const MAX_PAGE_BODY_REVISIONS_PER_BODY: usize = 50;

impl PageService {
    pub(super) async fn find_page_for_update(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<page::Model> {
        let query =
            || page::Entity::find_by_id(page_id).filter(page::Column::TenantId.eq(tenant_id));
        let page = match txn.get_database_backend() {
            DbBackend::Sqlite => query().one(txn).await?,
            DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(txn).await?,
            _ => unreachable!("unsupported SeaORM database backend"),
        };
        page.ok_or_else(|| PagesError::page_not_found(page_id))
    }

    pub(super) async fn ensure_slug_unique_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        slug: &str,
        exclude_page_id: Option<Uuid>,
    ) -> PagesResult<()> {
        let locale = normalize_locale(locale)?;
        let slug = normalize_slug(slug)?;
        let mut select = page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::Locale.eq(&locale))
            .filter(page_translation::Column::Slug.eq(&slug));
        if let Some(exclude_page_id) = exclude_page_id {
            select = select.filter(page_translation::Column::PageId.ne(exclude_page_id));
        }
        if select.one(txn).await?.is_some() {
            return Err(PagesError::duplicate_slug(&slug, &locale));
        }

        let snapshots = page_route_publication::Entity::find()
            .filter(page_route_publication::Column::TenantId.eq(tenant_id))
            .filter(page_route_publication::Column::Locale.eq(&locale))
            .filter(page_route_publication::Column::Slug.eq(&slug))
            .all(txn)
            .await?;
        match snapshots.as_slice() {
            [] => {}
            [snapshot] if Some(snapshot.page_id) == exclude_page_id => {}
            _ => return Err(PagesError::duplicate_slug(&slug, &locale)),
        }

        ensure_route_alias_claim_available_in_tx(txn, tenant_id, &locale, &slug).await
    }

    pub(super) async fn replace_translations_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
        translations: &[PageTranslationInput],
    ) -> PagesResult<()> {
        for translation in translations {
            let locale = normalize_locale(&translation.locale)?;
            let slug = normalize_slug(
                translation
                    .slug
                    .as_deref()
                    .unwrap_or(translation.title.as_str()),
            )?;
            let existing = page_translation::Entity::find()
                .filter(page_translation::Column::TenantId.eq(tenant_id))
                .filter(page_translation::Column::PageId.eq(page_id))
                .filter(page_translation::Column::Locale.eq(&locale))
                .one(txn)
                .await?;
            match existing {
                Some(existing) => {
                    let revision =
                        next_page_translation_revision(page_id, &locale, existing.revision)?;
                    let mut active: page_translation::ActiveModel = existing.into();
                    active.title = Set(translation.title.clone());
                    active.slug = Set(slug);
                    active.meta_title = Set(translation.meta_title.clone());
                    active.meta_description = Set(translation.meta_description.clone());
                    active.revision = Set(revision);
                    active.update(txn).await?;
                }
                None => {
                    page_translation::ActiveModel {
                        id: Set(Uuid::new_v4()),
                        page_id: Set(page_id),
                        tenant_id: Set(tenant_id),
                        locale: Set(locale),
                        title: Set(translation.title.clone()),
                        slug: Set(slug),
                        meta_title: Set(translation.meta_title.clone()),
                        meta_description: Set(translation.meta_description.clone()),
                        revision: Set(1),
                    }
                    .insert(txn)
                    .await?;
                }
            }
        }
        Ok(())
    }

    pub(super) async fn upsert_body_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
        body: Option<PreparedPageBody>,
        now: DateTimeWithTimeZone,
    ) -> PagesResult<()> {
        let Some(body) = body else {
            return Ok(());
        };
        let locale = normalize_locale(&body.locale)?;
        let existing = page_body::Entity::find()
            .filter(page_body::Column::TenantId.eq(tenant_id))
            .filter(page_body::Column::PageId.eq(page_id))
            .filter(page_body::Column::Locale.eq(&locale))
            .one(txn)
            .await?;
        match existing {
            Some(existing) => {
                let mut active: page_body::ActiveModel = existing.into();
                active.content = Set(body.content);
                active.format = Set(body.format);
                active.updated_at = Set(now);
                active.update(txn).await?;
            }
            None => {
                page_body::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    page_id: Set(page_id),
                    locale: Set(locale),
                    content: Set(body.content),
                    format: Set(body.format),
                    updated_at: Set(now),
                }
                .insert(txn)
                .await?;
            }
        }
        Ok(())
    }

    /// Upserts the unpublished working copy of one published-page locale.
    pub(super) async fn upsert_draft_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
        body: &PreparedPageBody,
        now: DateTimeWithTimeZone,
        created_by: Option<Uuid>,
    ) -> PagesResult<page_body_draft::Model> {
        let locale = normalize_locale(&body.locale)?;
        let query = || {
            page_body_draft::Entity::find()
                .filter(page_body_draft::Column::TenantId.eq(tenant_id))
                .filter(page_body_draft::Column::PageId.eq(page_id))
                .filter(page_body_draft::Column::Locale.eq(&locale))
        };
        let existing = match txn.get_database_backend() {
            DbBackend::Sqlite => query().one(txn).await?,
            DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(txn).await?,
            _ => unreachable!("unsupported SeaORM database backend"),
        };
        let model = match existing {
            Some(existing) => {
                let mut active: page_body_draft::ActiveModel = existing.into();
                active.content = Set(body.content.clone());
                active.format = Set(body.format.clone());
                active.updated_at = Set(now);
                active.update(txn).await?
            }
            None => {
                page_body_draft::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    page_id: Set(page_id),
                    locale: Set(locale),
                    content: Set(body.content.clone()),
                    format: Set(body.format.clone()),
                    created_at: Set(now),
                    updated_at: Set(now),
                    created_by: Set(created_by),
                }
                .insert(txn)
                .await?
            }
        };
        Ok(model)
    }

    pub(super) async fn find_draft_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
        locale: &str,
    ) -> PagesResult<Option<page_body_draft::Model>> {
        let query = || {
            page_body_draft::Entity::find()
                .filter(page_body_draft::Column::TenantId.eq(tenant_id))
                .filter(page_body_draft::Column::PageId.eq(page_id))
                .filter(page_body_draft::Column::Locale.eq(locale))
        };
        Ok(match txn.get_database_backend() {
            DbBackend::Sqlite => query().one(txn).await?,
            DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(txn).await?,
            _ => unreachable!("unsupported SeaORM database backend"),
        })
    }

    pub(super) async fn find_body_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
        locale: &str,
    ) -> PagesResult<Option<page_body::Model>> {
        let query = || {
            page_body::Entity::find()
                .filter(page_body::Column::TenantId.eq(tenant_id))
                .filter(page_body::Column::PageId.eq(page_id))
                .filter(page_body::Column::Locale.eq(locale))
        };
        Ok(match txn.get_database_backend() {
            DbBackend::Sqlite => query().one(txn).await?,
            DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(txn).await?,
            _ => unreachable!("unsupported SeaORM database backend"),
        })
    }

    /// Loads the locked working copies of every locale (current bodies overlaid by drafts).
    pub(super) async fn load_working_bodies_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<Vec<WorkingBody>> {
        let bodies_query = || {
            page_body::Entity::find()
                .filter(page_body::Column::TenantId.eq(tenant_id))
                .filter(page_body::Column::PageId.eq(page_id))
                .order_by_asc(page_body::Column::Locale)
        };
        let drafts_query = || {
            page_body_draft::Entity::find()
                .filter(page_body_draft::Column::TenantId.eq(tenant_id))
                .filter(page_body_draft::Column::PageId.eq(page_id))
                .order_by_asc(page_body_draft::Column::Locale)
        };
        let (bodies, drafts) = match txn.get_database_backend() {
            DbBackend::Sqlite => (
                bodies_query().all(txn).await?,
                drafts_query().all(txn).await?,
            ),
            DbBackend::Postgres | DbBackend::MySql => (
                bodies_query().lock_exclusive().all(txn).await?,
                drafts_query().lock_exclusive().all(txn).await?,
            ),
            _ => unreachable!("unsupported SeaORM database backend"),
        };
        Ok(merge_working_bodies(&bodies, &drafts))
    }

    /// Appends one immutable body-revision journal row and enforces the retention window.
    pub(super) async fn record_body_revision_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
        locale: &str,
        content: &str,
        format: &str,
        source: PageBodyRevisionSource,
        body_revision: &str,
        created_by: Option<Uuid>,
    ) -> PagesResult<page_body_revision::Model> {
        let now = Utc::now();
        let model = page_body_revision::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            page_id: Set(page_id),
            locale: Set(locale.to_string()),
            content: Set(content.to_string()),
            format: Set(format.to_string()),
            source: Set(source.as_str().to_string()),
            body_revision: Set(body_revision.to_string()),
            created_at: Set(now.into()),
            created_by: Set(created_by),
        }
        .insert(txn)
        .await?;
        self.prune_body_revisions_in_tx(txn, tenant_id, page_id, locale)
            .await?;
        Ok(model)
    }

    async fn prune_body_revisions_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
        locale: &str,
    ) -> PagesResult<()> {
        let keep = page_body_revision::Entity::find()
            .filter(page_body_revision::Column::TenantId.eq(tenant_id))
            .filter(page_body_revision::Column::PageId.eq(page_id))
            .filter(page_body_revision::Column::Locale.eq(locale))
            .order_by_desc(page_body_revision::Column::CreatedAt)
            .order_by_desc(page_body_revision::Column::Id)
            .limit(MAX_PAGE_BODY_REVISIONS_PER_BODY as u64)
            .all(txn)
            .await?
            .into_iter()
            .map(|model| model.id)
            .collect::<Vec<_>>();
        page_body_revision::Entity::delete_many()
            .filter(page_body_revision::Column::TenantId.eq(tenant_id))
            .filter(page_body_revision::Column::PageId.eq(page_id))
            .filter(page_body_revision::Column::Locale.eq(locale))
            .filter(page_body_revision::Column::Id.is_not_in(keep))
            .exec(txn)
            .await?;
        Ok(())
    }

    /// Moves every page body draft into its current body, preserving each draft revision token.
    ///
    /// Publishes, unpublishes and archives call this so a published page keeps exactly one
    /// editable working copy. Returns the promoted working copies.
    pub(super) async fn promote_drafts_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
        created_by: Option<Uuid>,
    ) -> PagesResult<Vec<WorkingBody>> {
        let query = || {
            page_body_draft::Entity::find()
                .filter(page_body_draft::Column::TenantId.eq(tenant_id))
                .filter(page_body_draft::Column::PageId.eq(page_id))
                .order_by_asc(page_body_draft::Column::Locale)
        };
        let drafts = match txn.get_database_backend() {
            DbBackend::Sqlite => query().all(txn).await?,
            DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().all(txn).await?,
            _ => unreachable!("unsupported SeaORM database backend"),
        };
        let mut promoted = Vec::with_capacity(drafts.len());
        for draft in drafts {
            let body_revision = draft.updated_at.to_string();
            let query = || {
                page_body::Entity::find()
                    .filter(page_body::Column::TenantId.eq(tenant_id))
                    .filter(page_body::Column::PageId.eq(page_id))
                    .filter(page_body::Column::Locale.eq(&draft.locale))
            };
            let existing = match txn.get_database_backend() {
                DbBackend::Sqlite => query().one(txn).await?,
                DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(txn).await?,
                _ => unreachable!("unsupported SeaORM database backend"),
            };
            match existing {
                Some(existing) => {
                    let mut active: page_body::ActiveModel = existing.into();
                    active.content = Set(draft.content.clone());
                    active.format = Set(draft.format.clone());
                    active.updated_at = Set(draft.updated_at);
                    active.update(txn).await?;
                }
                None => {
                    page_body::ActiveModel {
                        id: Set(Uuid::new_v4()),
                        tenant_id: Set(tenant_id),
                        page_id: Set(page_id),
                        locale: Set(draft.locale.clone()),
                        content: Set(draft.content.clone()),
                        format: Set(draft.format.clone()),
                        updated_at: Set(draft.updated_at),
                    }
                    .insert(txn)
                    .await?;
                }
            }
            self.record_body_revision_in_tx(
                txn,
                tenant_id,
                page_id,
                &draft.locale,
                &draft.content,
                &draft.format,
                PageBodyRevisionSource::Promote,
                &body_revision,
                created_by,
            )
            .await?;
            page_body_draft::Entity::delete_by_id(draft.id)
                .exec(txn)
                .await?;
            promoted.push(WorkingBody::from(&draft));
        }
        Ok(promoted)
    }

    pub(super) async fn replace_channel_visibility_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
        channel_slugs: &[String],
    ) -> PagesResult<()> {
        page_channel_visibility::Entity::delete_many()
            .filter(page_channel_visibility::Column::TenantId.eq(tenant_id))
            .filter(page_channel_visibility::Column::PageId.eq(page_id))
            .exec(txn)
            .await?;

        for channel_slug in channel_slugs {
            page_channel_visibility::ActiveModel {
                id: Set(Uuid::new_v4()),
                page_id: Set(page_id),
                tenant_id: Set(tenant_id),
                channel_slug: Set(channel_slug.clone()),
                created_at: Set(Utc::now().into()),
            }
            .insert(txn)
            .await?;
        }

        Ok(())
    }
}
