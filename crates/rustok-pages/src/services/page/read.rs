use std::collections::HashMap;

use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource};
use rustok_content::entities::node::ContentStatus;
use rustok_core::SecurityContext;

use crate::dto::{ListPagesFilter, PageListItem, PageResponse};
use crate::entities::{page, page_body, page_channel_visibility, page_translation};
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::{can_read_non_public_pages, enforce_scope};

use super::helpers::{
    apply_public_page_channel_filter, available_locales, body_for_locale, normalize_locale,
    normalize_slug, page_body_response, page_translation_response, resolve_translation_record,
    status_to_storage, storage_to_status,
};
use super::{PageResponseParts, PageService};

impl PageService {
    #[instrument(skip(self))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PageResponse> {
        self.get_with_locale_fallback(tenant_id, security, page_id, PLATFORM_FALLBACK_LOCALE, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> PagesResult<PageResponse> {
        enforce_scope(&security, Resource::Pages, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let page = self.find_page(tenant_id, page_id).await?;
        if !can_read_non_public_pages(&security)
            && storage_to_status(&page.status)? != ContentStatus::Published
        {
            return Err(PagesError::forbidden("Permission denied"));
        }
        let channel_slugs = self.load_channel_slugs(tenant_id, page_id).await?;
        let translations = self.load_translations(tenant_id, page_id).await?;
        let bodies = self.load_bodies(tenant_id, page_id).await?;
        self.build_page_response(
            page,
            translations,
            bodies,
            PageResponseParts {
                channel_slugs,
                locale,
                fallback_locale,
            },
        )
    }

    #[instrument(skip(self))]
    pub async fn get_by_slug_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        slug: &str,
        fallback_locale: Option<&str>,
    ) -> PagesResult<Option<PageResponse>> {
        enforce_scope(&security, Resource::Pages, Action::Read)?;
        let requested_locale = normalize_locale(locale)?;
        let normalized_fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let candidates = page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::Slug.eq(normalize_slug(slug)?))
            .all(&self.db)
            .await?;
        let resolved = resolve_translation_record(
            &candidates,
            &requested_locale,
            normalized_fallback_locale.as_deref(),
        );
        let Some(translation) = resolved.translation else {
            return Ok(None);
        };

        let page = self.find_page(tenant_id, translation.page_id).await?;
        if storage_to_status(&page.status)? != ContentStatus::Published {
            return Ok(None);
        }
        let channel_slugs = self.load_channel_slugs(tenant_id, page.id).await?;
        let translations = self.load_translations(tenant_id, page.id).await?;
        let bodies = self.load_bodies(tenant_id, page.id).await?;
        self.build_page_response(
            page,
            translations,
            bodies,
            PageResponseParts {
                channel_slugs,
                locale: requested_locale,
                fallback_locale: normalized_fallback_locale,
            },
        )
        .map(Some)
    }

    #[instrument(skip(self))]
    pub async fn get_by_slug(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        slug: &str,
    ) -> PagesResult<Option<PageResponse>> {
        self.get_by_slug_with_locale_fallback(tenant_id, security, locale, slug, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListPagesFilter,
    ) -> PagesResult<(Vec<PageListItem>, u64)> {
        enforce_scope(&security, Resource::Pages, Action::List)?;
        let locale = filter
            .locale
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let mut select = page::Entity::find().filter(page::Column::TenantId.eq(tenant_id));
        if !can_read_non_public_pages(&security) {
            if matches!(
                filter.status,
                Some(ref status) if status != &ContentStatus::Published
            ) {
                return Ok((Vec::new(), 0));
            }
            select = select
                .filter(page::Column::Status.eq(status_to_storage(&ContentStatus::Published)));
        }
        if let Some(status) = filter.status {
            select = select.filter(page::Column::Status.eq(status_to_storage(&status)));
        }
        if let Some(template) = filter.template {
            select = select.filter(page::Column::Template.eq(template));
        }
        self.page_list_from_select(tenant_id, select, locale, filter.page, filter.per_page)
            .await
    }

    #[instrument(skip(self))]
    pub async fn list_public_visible(
        &self,
        tenant_id: Uuid,
        filter: ListPagesFilter,
        channel_slug: Option<&str>,
    ) -> PagesResult<(Vec<PageListItem>, u64)> {
        let locale = filter
            .locale
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let mut select = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Status.eq(status_to_storage(&ContentStatus::Published)));
        if let Some(template) = filter.template {
            select = select.filter(page::Column::Template.eq(template));
        }
        select = apply_public_page_channel_filter(select, tenant_id, channel_slug);
        self.page_list_from_select(tenant_id, select, locale, filter.page, filter.per_page)
            .await
    }

    async fn page_list_from_select(
        &self,
        tenant_id: Uuid,
        select: sea_orm::Select<page::Entity>,
        locale: String,
        page_number: u64,
        per_page: u64,
    ) -> PagesResult<(Vec<PageListItem>, u64)> {
        let paginator = select
            .order_by_desc(page::Column::UpdatedAt)
            .paginate(&self.db, per_page.max(1));
        let total = paginator.num_items().await?;
        let pages = paginator.fetch_page(page_number.saturating_sub(1)).await?;
        let page_ids: Vec<Uuid> = pages.iter().map(|item| item.id).collect();
        let translations_map = self.load_translations_map(tenant_id, &page_ids).await?;
        let channel_slugs_map = self.load_channel_slugs_map(tenant_id, &page_ids).await?;

        let mut items = Vec::with_capacity(pages.len());
        for page in pages {
            let translations = translations_map.get(&page.id).cloned().unwrap_or_default();
            let resolved = resolve_translation_record(&translations, &locale, None);
            items.push(PageListItem {
                id: page.id,
                status: storage_to_status(&page.status)?,
                template: page.template.clone(),
                title: resolved.translation.map(|item| item.title.clone()),
                slug: resolved.translation.map(|item| item.slug.clone()),
                channel_slugs: channel_slugs_map.get(&page.id).cloned().unwrap_or_default(),
                updated_at: page.updated_at.to_string(),
            });
        }

        Ok((items, total))
    }

    pub(super) async fn find_page(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<page::Model> {
        page::Entity::find_by_id(page_id)
            .filter(page::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| PagesError::page_not_found(page_id))
    }

    pub(super) async fn load_translations(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<Vec<page_translation::Model>> {
        Ok(page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::PageId.eq(page_id))
            .all(&self.db)
            .await?)
    }

    async fn load_translations_map(
        &self,
        tenant_id: Uuid,
        page_ids: &[Uuid],
    ) -> PagesResult<HashMap<Uuid, Vec<page_translation::Model>>> {
        if page_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let translations = page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::PageId.is_in(page_ids.to_vec()))
            .all(&self.db)
            .await?;
        let mut map: HashMap<Uuid, Vec<page_translation::Model>> = HashMap::new();
        for translation in translations {
            map.entry(translation.page_id)
                .or_default()
                .push(translation);
        }
        Ok(map)
    }

    pub(super) async fn load_channel_slugs(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<Vec<String>> {
        let records = page_channel_visibility::Entity::find()
            .filter(page_channel_visibility::Column::TenantId.eq(tenant_id))
            .filter(page_channel_visibility::Column::PageId.eq(page_id))
            .order_by_asc(page_channel_visibility::Column::ChannelSlug)
            .all(&self.db)
            .await?;
        Ok(records.into_iter().map(|item| item.channel_slug).collect())
    }

    async fn load_channel_slugs_map(
        &self,
        tenant_id: Uuid,
        page_ids: &[Uuid],
    ) -> PagesResult<HashMap<Uuid, Vec<String>>> {
        if page_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let records = page_channel_visibility::Entity::find()
            .filter(page_channel_visibility::Column::TenantId.eq(tenant_id))
            .filter(page_channel_visibility::Column::PageId.is_in(page_ids.to_vec()))
            .order_by_asc(page_channel_visibility::Column::ChannelSlug)
            .all(&self.db)
            .await?;
        let mut map: HashMap<Uuid, Vec<String>> = HashMap::new();
        for record in records {
            map.entry(record.page_id)
                .or_default()
                .push(record.channel_slug);
        }
        Ok(map)
    }

    pub(super) async fn load_bodies(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<Vec<page_body::Model>> {
        Ok(page_body::Entity::find()
            .filter(page_body::Column::TenantId.eq(tenant_id))
            .filter(page_body::Column::PageId.eq(page_id))
            .all(&self.db)
            .await?)
    }

    fn build_page_response(
        &self,
        page: page::Model,
        translations: Vec<page_translation::Model>,
        bodies: Vec<page_body::Model>,
        parts: PageResponseParts,
    ) -> PagesResult<PageResponse> {
        let translation = resolve_translation_record(
            &translations,
            parts.locale.as_str(),
            parts.fallback_locale.as_deref(),
        );
        let response_body = translation
            .translation
            .and_then(|_| body_for_locale(&bodies, translation.effective_locale.as_str()))
            .map(page_body_response);
        let effective_locale = translation
            .translation
            .map(|_| translation.effective_locale.clone());
        Ok(PageResponse {
            id: page.id,
            version: page.version,
            status: storage_to_status(&page.status)?,
            requested_locale: Some(parts.locale),
            effective_locale,
            available_locales: available_locales(&translations),
            template: page.template,
            created_at: page.created_at.to_string(),
            updated_at: page.updated_at.to_string(),
            published_at: page.published_at.map(|value| value.to_string()),
            translation: translation.translation.map(page_translation_response),
            translations: translations.iter().map(page_translation_response).collect(),
            body: response_body,
            channel_slugs: parts.channel_slugs,
            metadata: page.metadata,
        })
    }
}
