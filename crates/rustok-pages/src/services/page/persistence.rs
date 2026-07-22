use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseTransaction,
    DbBackend, EntityTrait, QueryFilter, QuerySelect,
};
use uuid::Uuid;

use crate::dto::PageTranslationInput;
use crate::entities::{page, page_body, page_channel_visibility, page_translation};
use crate::error::{PagesError, PagesResult};

use super::helpers::{normalize_locale, normalize_slug};
use super::{PageService, PreparedPageBody};

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
        let mut select = page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::Locale.eq(normalize_locale(locale)?))
            .filter(page_translation::Column::Slug.eq(slug));
        if let Some(exclude_page_id) = exclude_page_id {
            select = select.filter(page_translation::Column::PageId.ne(exclude_page_id));
        }
        if select.one(txn).await?.is_some() {
            return Err(PagesError::duplicate_slug(slug, locale));
        }
        Ok(())
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
            );
            let existing = page_translation::Entity::find()
                .filter(page_translation::Column::PageId.eq(page_id))
                .filter(page_translation::Column::Locale.eq(&locale))
                .one(txn)
                .await?;
            match existing {
                Some(existing) => {
                    let mut active: page_translation::ActiveModel = existing.into();
                    active.title = Set(translation.title.clone());
                    active.slug = Set(slug);
                    active.meta_title = Set(translation.meta_title.clone());
                    active.meta_description = Set(translation.meta_description.clone());
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
        page_id: Uuid,
        body: Option<PreparedPageBody>,
        now: chrono::DateTime<Utc>,
    ) -> PagesResult<()> {
        let Some(body) = body else {
            return Ok(());
        };
        let locale = normalize_locale(&body.locale)?;
        let existing = page_body::Entity::find()
            .filter(page_body::Column::PageId.eq(page_id))
            .filter(page_body::Column::Locale.eq(&locale))
            .one(txn)
            .await?;
        match existing {
            Some(existing) => {
                let mut active: page_body::ActiveModel = existing.into();
                active.content = Set(body.content);
                active.format = Set(body.format);
                active.updated_at = Set(now.into());
                active.update(txn).await?;
            }
            None => {
                page_body::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    page_id: Set(page_id),
                    locale: Set(locale),
                    content: Set(body.content),
                    format: Set(body.format),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;
            }
        }
        Ok(())
    }

    pub(super) async fn replace_channel_visibility_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
        channel_slugs: &[String],
    ) -> PagesResult<()> {
        page_channel_visibility::Entity::delete_many()
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
