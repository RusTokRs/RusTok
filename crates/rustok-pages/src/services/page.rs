use chrono::Utc;
use sea_orm::{
    sea_query::{Expr, Query, SelectStatement},
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, Condition, DatabaseConnection, DatabaseTransaction, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Select, TransactionTrait,
};
use std::collections::HashMap;
use tracing::instrument;
use uuid::Uuid;

use rustok_content::{
    available_locales_from, normalize_locale_code, resolve_by_locale_with_fallback,
};
use rustok_core::{
    normalize_content_format, prepare_content_payload, Action, Resource, SecurityContext,
    CONTENT_FORMAT_GRAPESJS_V1, CONTENT_FORMAT_RT_JSON_V1,
};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::*;
use crate::entities::{page, page_body, page_channel_visibility, page_translation};
use crate::error::{
    PagesError, PagesResult, FEATURE_BUILDER_ENABLED, FEATURE_BUILDER_PUBLISH_ENABLED,
};
use crate::services::rbac::{can_read_non_public_pages, enforce_owned_scope, enforce_scope};
use crate::services::BlockService;
use rustok_tenant::entities::tenant_module;

const PAGE_KIND: &str = "page";
const PLATFORM_FALLBACK_LOCALE: &str = "en";

struct PageResponseParts {
    channel_slugs: Vec<String>,
    blocks: Vec<BlockResponse>,
    locale: String,
    fallback_locale: Option<String>,
}

pub struct PageService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    blocks: BlockService,
}

struct PreparedPageBody {
    locale: String,
    content: String,
    format: String,
}

struct ResolvedTranslationRecord<'a> {
    translation: Option<&'a page_translation::Model>,
    effective_locale: String,
}

struct ResolvedBodyRecord<'a> {
    body: Option<&'a page_body::Model>,
    effective_locale: String,
}

impl PageService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            db: db.clone(),
            event_bus: event_bus.clone(),
            blocks: BlockService::new(db, event_bus),
        }
    }

    #[instrument(skip(self, input))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreatePageInput,
    ) -> PagesResult<PageResponse> {
        enforce_scope(&security, Resource::Pages, Action::Create)?;
        if input.publish {
            enforce_scope(&security, Resource::Pages, Action::Publish)?;
            self.ensure_builder_publish_enabled(tenant_id).await?;
        }
        validate_page_translations(&input.translations)?;
        let template = input
            .template
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let metadata = build_page_metadata(&template, &input.translations, None);
        let channel_slugs = normalize_channel_slugs(input.channel_slugs.as_deref().unwrap_or(&[]));
        let body = normalize_page_body_input(input.body)?;
        if body_uses_builder_capability(body.as_ref()) {
            self.ensure_builder_enabled(tenant_id).await?;
        }
        let now = Utc::now();
        let page_id = Uuid::new_v4();

        let txn = self.db.begin().await?;
        for translation in &input.translations {
            let slug = normalize_slug(
                translation
                    .slug
                    .as_deref()
                    .unwrap_or(translation.title.as_str()),
            );
            self.ensure_slug_unique_in_tx(&txn, tenant_id, &translation.locale, &slug, None)
                .await?;
        }

        let initial_status = if input.publish {
            rustok_content::entities::node::ContentStatus::Published
        } else {
            rustok_content::entities::node::ContentStatus::Draft
        };

        page::ActiveModel {
            id: Set(page_id),
            tenant_id: Set(tenant_id),
            author_id: Set(security.user_id),
            status: Set(status_to_storage(&initial_status).to_string()),
            template: Set(template),
            metadata: Set(metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            published_at: Set(if input.publish {
                Some(now.into())
            } else {
                None
            }),
            archived_at: Set(None),
            version: Set(1),
        }
        .insert(&txn)
        .await?;

        self.replace_translations_in_tx(&txn, tenant_id, page_id, &input.translations)
            .await?;
        self.replace_channel_visibility_in_tx(&txn, tenant_id, page_id, &channel_slugs)
            .await?;
        self.upsert_body_in_tx(&txn, page_id, body, now).await?;
        if let Some(blocks) = input.blocks {
            for block in blocks {
                BlockService::create_in_tx(&txn, tenant_id, security.clone(), page_id, block)
                    .await?;
            }
        }

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::NodeCreated {
                    node_id: page_id,
                    kind: PAGE_KIND.to_string(),
                    author_id: security.user_id,
                },
            )
            .await?;

        txn.commit().await?;
        self.get(tenant_id, security, page_id).await
    }

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
            && storage_to_status(&page.status)?
                != rustok_content::entities::node::ContentStatus::Published
        {
            return Err(PagesError::forbidden("Permission denied"));
        }
        let channel_slugs = self.load_channel_slugs(page_id).await?;
        let translations = self.load_translations(page_id).await?;
        let bodies = self.load_bodies(page_id).await?;
        let blocks = self
            .blocks
            .list_for_page(tenant_id, security, page_id)
            .await?;
        self.build_page_response(
            page,
            translations,
            bodies,
            PageResponseParts {
                channel_slugs,
                blocks,
                locale: locale.clone(),
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
            .filter(page_translation::Column::Slug.eq(normalize_slug(slug)))
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
        if storage_to_status(&page.status)?
            != rustok_content::entities::node::ContentStatus::Published
        {
            return Ok(None);
        }
        let channel_slugs = self.load_channel_slugs(page.id).await?;
        let translations = self.load_translations(page.id).await?;
        let bodies = self.load_bodies(page.id).await?;
        let blocks = self
            .blocks
            .list_for_page(tenant_id, security, page.id)
            .await?;
        self.build_page_response(
            page,
            translations,
            bodies,
            PageResponseParts {
                channel_slugs,
                blocks,
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
                Some(ref status)
                    if status != &rustok_content::entities::node::ContentStatus::Published
            ) {
                return Ok((Vec::new(), 0));
            }
            select = select.filter(page::Column::Status.eq(status_to_storage(
                &rustok_content::entities::node::ContentStatus::Published,
            )));
        }
        if let Some(status) = filter.status {
            select = select.filter(page::Column::Status.eq(status_to_storage(&status)));
        }
        if let Some(template) = filter.template {
            select = select.filter(page::Column::Template.eq(template));
        }
        let paginator = select
            .order_by_desc(page::Column::UpdatedAt)
            .paginate(&self.db, filter.per_page.max(1));
        let total = paginator.num_items().await?;
        let pages = paginator.fetch_page(filter.page.saturating_sub(1)).await?;
        let page_ids: Vec<Uuid> = pages.iter().map(|item| item.id).collect();
        let translations_map = self.load_translations_map(&page_ids).await?;
        let channel_slugs_map = self.load_channel_slugs_map(&page_ids).await?;

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
            .filter(page::Column::Status.eq(status_to_storage(
                &rustok_content::entities::node::ContentStatus::Published,
            )));
        if let Some(template) = filter.template {
            select = select.filter(page::Column::Template.eq(template));
        }
        select = apply_public_page_channel_filter(select, tenant_id, channel_slug);

        let paginator = select
            .order_by_desc(page::Column::UpdatedAt)
            .paginate(&self.db, filter.per_page.max(1));
        let total = paginator.num_items().await?;
        let pages = paginator.fetch_page(filter.page.saturating_sub(1)).await?;
        let page_ids: Vec<Uuid> = pages.iter().map(|item| item.id).collect();
        let translations_map = self.load_translations_map(&page_ids).await?;
        let channel_slugs_map = self.load_channel_slugs_map(&page_ids).await?;

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

    #[instrument(skip(self, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: UpdatePageInput,
    ) -> PagesResult<PageResponse> {
        let existing = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            existing.author_id,
        )?;
        if input.status.is_some() {
            enforce_scope(&security, Resource::Pages, Action::Publish)?;
            if matches!(
                input.status,
                Some(rustok_content::entities::node::ContentStatus::Published)
            ) {
                self.ensure_builder_enabled_for_page(tenant_id, page_id).await?;
                self.ensure_builder_publish_enabled(tenant_id).await?;
            }
        }
        if let Some(ref translations) = input.translations {
            validate_page_translations(translations)?;
        }

        let template = input
            .template
            .clone()
            .unwrap_or_else(|| existing.template.clone());
        let metadata = build_page_metadata(
            &template,
            input.translations.as_deref().unwrap_or(&[]),
            Some(&existing.metadata),
        );
        let channel_slugs = input
            .channel_slugs
            .as_ref()
            .map(|items| normalize_channel_slugs(items))
            .unwrap_or_default();
        let replace_channel_visibility = input.channel_slugs.is_some();
        let body = normalize_page_body_input(input.body)?;
        if body_uses_builder_capability(body.as_ref()) {
            self.ensure_builder_enabled(tenant_id).await?;
        }
        let locale = input
            .translations
            .as_ref()
            .and_then(|items| items.first().map(|item| item.locale.clone()))
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());

        let txn = self.db.begin().await?;
        if let Some(ref translations) = input.translations {
            for translation in translations {
                let slug = normalize_slug(
                    translation
                        .slug
                        .as_deref()
                        .unwrap_or(translation.title.as_str()),
                );
                self.ensure_slug_unique_in_tx(
                    &txn,
                    tenant_id,
                    &translation.locale,
                    &slug,
                    Some(page_id),
                )
                .await?;
            }
        }

        let mut active: page::ActiveModel = existing.into();
        active.template = Set(template);
        active.metadata = Set(metadata);
        active.updated_at = Set(Utc::now().into());
        active.version = Set(active.version.take().unwrap_or(1) + 1);
        if let Some(status) = input.status {
            active.status = Set(status_to_storage(&status).to_string());
        }
        active.update(&txn).await?;

        if let Some(ref translations) = input.translations {
            self.replace_translations_in_tx(&txn, tenant_id, page_id, translations)
                .await?;
        }
        if replace_channel_visibility {
            self.replace_channel_visibility_in_tx(&txn, tenant_id, page_id, &channel_slugs)
                .await?;
        }
        self.upsert_body_in_tx(&txn, page_id, body, Utc::now())
            .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::NodeUpdated {
                    node_id: page_id,
                    kind: PAGE_KIND.to_string(),
                },
            )
            .await?;
        txn.commit().await?;
        self.get_with_locale_fallback(
            tenant_id,
            security,
            page_id,
            &locale,
            Some(PLATFORM_FALLBACK_LOCALE),
        )
        .await
    }

    #[instrument(skip(self))]
    pub async fn publish(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PageResponse> {
        let existing = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            existing.author_id,
        )?;
        self.ensure_builder_enabled_for_page(tenant_id, page_id).await?;
        self.ensure_builder_publish_enabled(tenant_id).await?;
        self.set_status(
            tenant_id,
            security,
            page_id,
            rustok_content::entities::node::ContentStatus::Published,
            Some(DomainEvent::NodePublished {
                node_id: page_id,
                kind: PAGE_KIND.to_string(),
            }),
        )
        .await
    }

    #[instrument(skip(self))]
    pub async fn unpublish(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PageResponse> {
        self.set_status(
            tenant_id,
            security,
            page_id,
            rustok_content::entities::node::ContentStatus::Draft,
            Some(DomainEvent::NodeUnpublished {
                node_id: page_id,
                kind: PAGE_KIND.to_string(),
            }),
        )
        .await
    }

    pub async fn delete(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<()> {
        let existing = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Delete,
            existing.author_id,
        )?;
        let txn = self.db.begin().await?;
        BlockService::delete_all_for_page_in_tx(&txn, tenant_id, page_id).await?;
        page_body::Entity::delete_many()
            .filter(page_body::Column::PageId.eq(page_id))
            .exec(&txn)
            .await?;
        page_translation::Entity::delete_many()
            .filter(page_translation::Column::PageId.eq(page_id))
            .exec(&txn)
            .await?;
        page::Entity::delete_by_id(page_id).exec(&txn).await?;
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::NodeDeleted {
                    node_id: page_id,
                    kind: PAGE_KIND.to_string(),
                },
            )
            .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn set_status(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        status: rustok_content::entities::node::ContentStatus,
        follow_up_event: Option<DomainEvent>,
    ) -> PagesResult<PageResponse> {
        let existing = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            existing.author_id,
        )?;
        let txn = self.db.begin().await?;
        let mut active: page::ActiveModel = existing.into();
        active.status = Set(status_to_storage(&status).to_string());
        active.updated_at = Set(Utc::now().into());
        active.version = Set(active.version.take().unwrap_or(1) + 1);
        if matches!(
            status,
            rustok_content::entities::node::ContentStatus::Published
        ) {
            active.published_at = Set(Some(Utc::now().into()));
            active.archived_at = Set(None);
        } else {
            active.published_at = Set(None);
            active.archived_at = Set(None);
        }
        active.update(&txn).await?;
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::NodeUpdated {
                    node_id: page_id,
                    kind: PAGE_KIND.to_string(),
                },
            )
            .await?;
        if let Some(event) = follow_up_event {
            self.event_bus
                .publish_in_tx(&txn, tenant_id, security.user_id, event)
                .await?;
        }
        txn.commit().await?;
        self.get(tenant_id, security, page_id).await
    }

    async fn ensure_builder_publish_enabled(&self, tenant_id: Uuid) -> PagesResult<()> {
        let module = self.load_tenant_pages_module(tenant_id).await?;
        let enabled = module
            .as_ref()
            .map(|m| is_builder_publish_enabled(&m.settings))
            .unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(FEATURE_BUILDER_PUBLISH_ENABLED));
        }
        Ok(())
    }

    async fn ensure_builder_enabled(&self, tenant_id: Uuid) -> PagesResult<()> {
        let module = self.load_tenant_pages_module(tenant_id).await?;
        let enabled = module
            .as_ref()
            .map(|m| is_builder_enabled(&m.settings))
            .unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(FEATURE_BUILDER_ENABLED));
        }
        Ok(())
    }

    async fn ensure_builder_enabled_for_page(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<()> {
        let page = self.find_page(tenant_id, page_id).await?;
        let bodies = self.load_bodies(page.id).await?;
        if page_uses_builder_capability(&bodies) {
            self.ensure_builder_enabled(tenant_id).await?;
        }
        Ok(())
    }

    async fn load_tenant_pages_module(
        &self,
        tenant_id: Uuid,
    ) -> PagesResult<Option<tenant_module::Model>> {
        let module = tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .filter(tenant_module::Column::ModuleSlug.eq("pages"))
            .one(&self.db)
            .await?;
        Ok(module)
    }

    async fn find_page(&self, tenant_id: Uuid, page_id: Uuid) -> PagesResult<page::Model> {
        page::Entity::find_by_id(page_id)
            .filter(page::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| PagesError::page_not_found(page_id))
    }

    async fn load_translations(&self, page_id: Uuid) -> PagesResult<Vec<page_translation::Model>> {
        Ok(page_translation::Entity::find()
            .filter(page_translation::Column::PageId.eq(page_id))
            .all(&self.db)
            .await?)
    }

    async fn load_translations_map(
        &self,
        page_ids: &[Uuid],
    ) -> PagesResult<HashMap<Uuid, Vec<page_translation::Model>>> {
        if page_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let translations = page_translation::Entity::find()
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

    async fn load_channel_slugs(&self, page_id: Uuid) -> PagesResult<Vec<String>> {
        let records = page_channel_visibility::Entity::find()
            .filter(page_channel_visibility::Column::PageId.eq(page_id))
            .order_by_asc(page_channel_visibility::Column::ChannelSlug)
            .all(&self.db)
            .await?;
        Ok(records.into_iter().map(|item| item.channel_slug).collect())
    }

    async fn load_channel_slugs_map(
        &self,
        page_ids: &[Uuid],
    ) -> PagesResult<HashMap<Uuid, Vec<String>>> {
        if page_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let records = page_channel_visibility::Entity::find()
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

    async fn load_bodies(&self, page_id: Uuid) -> PagesResult<Vec<page_body::Model>> {
        Ok(page_body::Entity::find()
            .filter(page_body::Column::PageId.eq(page_id))
            .all(&self.db)
            .await?)
    }

    async fn ensure_slug_unique_in_tx(
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

    async fn replace_translations_in_tx(
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

    async fn upsert_body_in_tx(
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

    async fn replace_channel_visibility_in_tx(
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
        let body = resolve_body_record(
            &bodies,
            parts.locale.as_str(),
            parts.fallback_locale.as_deref(),
        );
        let response_body = body.body.map(page_body_response);
        let effective_locale = if response_body.is_some() {
            Some(body.effective_locale.clone())
        } else if translation.translation.is_some() {
            Some(translation.effective_locale.clone())
        } else {
            None
        };
        Ok(PageResponse {
            id: page.id,
            status: storage_to_status(&page.status)?,
            requested_locale: Some(parts.locale),
            effective_locale,
            available_locales: available_locales_from(&translations, |item| item.locale.as_str()),
            template: page.template,
            created_at: page.created_at.to_string(),
            updated_at: page.updated_at.to_string(),
            published_at: page.published_at.map(|value| value.to_string()),
            translation: translation.translation.map(page_translation_response),
            translations: translations.iter().map(page_translation_response).collect(),
            body: response_body,
            channel_slugs: parts.channel_slugs,
            blocks: parts.blocks,
            metadata: page.metadata,
        })
    }
}

fn validate_page_translations(translations: &[PageTranslationInput]) -> PagesResult<()> {
    if translations.is_empty() {
        return Err(PagesError::validation(
            "At least one page translation is required",
        ));
    }
    for translation in translations {
        if translation.locale.trim().is_empty() {
            return Err(PagesError::validation("Translation locale cannot be empty"));
        }
        if translation.title.trim().is_empty() {
            return Err(PagesError::validation("Page title cannot be empty"));
        }
    }
    Ok(())
}

fn normalize_page_body_input(body: Option<PageBodyInput>) -> PagesResult<Option<PreparedPageBody>> {
    let Some(body) = body else {
        return Ok(None);
    };
    let format =
        normalize_content_format(body.format.as_deref()).map_err(PagesError::validation)?;
    if body_requires_json_payload(&format)
        && body.content_json.is_none()
        && body.content.trim().is_empty()
    {
        return Err(PagesError::validation(format!(
            "content_json is required for {format} format"
        )));
    }
    let markdown_source = if body.content.trim().is_empty() {
        None
    } else {
        Some(body.content.as_str())
    };
    let prepared_body = prepare_content_payload(
        Some(&format),
        markdown_source,
        body.content_json.as_ref(),
        &body.locale,
        "Body",
    )
    .map_err(PagesError::validation)?;
    Ok(Some(PreparedPageBody {
        locale: body.locale,
        content: prepared_body.body,
        format: prepared_body.format,
    }))
}

fn normalize_locale(locale: &str) -> PagesResult<String> {
    normalize_locale_code(locale).ok_or_else(|| PagesError::validation("Invalid locale"))
}

fn normalize_slug(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            normalized.push('-');
            previous_dash = true;
        }
    }
    normalized.trim_matches('-').to_string()
}

fn is_builder_publish_enabled(settings: &serde_json::Value) -> bool {
    settings
        .get("builder")
        .and_then(|builder| builder.get("publish"))
        .and_then(|publish| publish.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

fn is_builder_enabled(settings: &serde_json::Value) -> bool {
    settings
        .get("builder")
        .and_then(|builder| builder.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

fn is_builder_preview_enabled(settings: &serde_json::Value) -> bool {
    settings
        .get("builder")
        .and_then(|builder| builder.get("preview"))
        .and_then(|preview| preview.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

fn is_builder_properties_enabled(settings: &serde_json::Value) -> bool {
    settings
        .get("builder")
        .and_then(|builder| builder.get("properties"))
        .and_then(|properties| properties.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

fn body_uses_builder_capability(body: Option<&PreparedPageBody>) -> bool {
    body.is_some_and(|item| item.format == CONTENT_FORMAT_GRAPESJS_V1)
}

fn page_uses_builder_capability(bodies: &[page_body::Model]) -> bool {
    bodies
        .iter()
        .any(|item| item.format == CONTENT_FORMAT_GRAPESJS_V1)
}

fn resolve_translation_record<'a>(
    translations: &'a [page_translation::Model],
    requested: &str,
    fallback_locale: Option<&str>,
) -> ResolvedTranslationRecord<'a> {
    let resolved =
        resolve_by_locale_with_fallback(translations, requested, fallback_locale, |item| {
            item.locale.as_str()
        });
    ResolvedTranslationRecord {
        translation: resolved.item,
        effective_locale: resolved.effective_locale,
    }
}

fn resolve_body_record<'a>(
    bodies: &'a [page_body::Model],
    requested: &str,
    fallback_locale: Option<&str>,
) -> ResolvedBodyRecord<'a> {
    let resolved = resolve_by_locale_with_fallback(bodies, requested, fallback_locale, |item| {
        item.locale.as_str()
    });
    ResolvedBodyRecord {
        body: resolved.item,
        effective_locale: resolved.effective_locale,
    }
}

fn storage_to_status(status: &str) -> PagesResult<rustok_content::entities::node::ContentStatus> {
    Ok(match status {
        "draft" => rustok_content::entities::node::ContentStatus::Draft,
        "published" => rustok_content::entities::node::ContentStatus::Published,
        "archived" => rustok_content::entities::node::ContentStatus::Archived,
        other => {
            return Err(PagesError::validation(format!(
                "Unknown page status: {other}"
            )))
        }
    })
}

fn status_to_storage(status: &rustok_content::entities::node::ContentStatus) -> &'static str {
    match status {
        rustok_content::entities::node::ContentStatus::Draft => "draft",
        rustok_content::entities::node::ContentStatus::Published => "published",
        rustok_content::entities::node::ContentStatus::Archived => "archived",
    }
}

fn build_page_metadata(
    template: &str,
    translations: &[PageTranslationInput],
    existing: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut metadata = existing
        .cloned()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    metadata["template"] = serde_json::json!(template);

    let mut seo = serde_json::Map::new();
    for translation in translations {
        if translation.meta_title.is_some() || translation.meta_description.is_some() {
            seo.insert(
                translation.locale.clone(),
                serde_json::json!({
                    "meta_title": translation.meta_title,
                    "meta_description": translation.meta_description,
                }),
            );
        }
    }
    if !seo.is_empty() {
        metadata["seo"] = serde_json::Value::Object(seo);
    } else if let Some(existing) = existing.and_then(|value| value.get("seo")) {
        metadata["seo"] = existing.clone();
    }

    metadata
}

pub(crate) fn is_page_visible_for_channel(
    channel_slugs: &[String],
    channel_slug: Option<&str>,
) -> bool {
    if channel_slugs.is_empty() {
        return true;
    }
    let Some(channel_slug) = channel_slug else {
        return false;
    };
    let normalized = channel_slug.trim().to_ascii_lowercase();
    !normalized.is_empty() && channel_slugs.iter().any(|item| item == &normalized)
}

fn normalize_channel_slugs(channel_slugs: &[String]) -> Vec<String> {
    let mut normalized = channel_slugs
        .iter()
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn apply_public_page_channel_filter(
    select: Select<page::Entity>,
    tenant_id: Uuid,
    channel_slug: Option<&str>,
) -> Select<page::Entity> {
    let unrestricted = Expr::col((page::Entity, page::Column::Id))
        .not_in_subquery(all_page_channel_visibility_subquery(tenant_id));
    let condition = match normalize_public_channel_slug(channel_slug) {
        Some(channel_slug) => Condition::any().add(unrestricted).add(
            Expr::col((page::Entity, page::Column::Id)).in_subquery(
                matching_page_channel_visibility_subquery(tenant_id, &channel_slug),
            ),
        ),
        None => Condition::all().add(unrestricted),
    };

    select.filter(condition)
}

fn all_page_channel_visibility_subquery(tenant_id: Uuid) -> SelectStatement {
    Query::select()
        .column(page_channel_visibility::Column::PageId)
        .from(page_channel_visibility::Entity)
        .and_where(
            Expr::col((
                page_channel_visibility::Entity,
                page_channel_visibility::Column::TenantId,
            ))
            .eq(tenant_id),
        )
        .to_owned()
}

fn matching_page_channel_visibility_subquery(
    tenant_id: Uuid,
    channel_slug: &str,
) -> SelectStatement {
    Query::select()
        .column(page_channel_visibility::Column::PageId)
        .from(page_channel_visibility::Entity)
        .and_where(
            Expr::col((
                page_channel_visibility::Entity,
                page_channel_visibility::Column::TenantId,
            ))
            .eq(tenant_id),
        )
        .and_where(
            Expr::col((
                page_channel_visibility::Entity,
                page_channel_visibility::Column::ChannelSlug,
            ))
            .eq(channel_slug),
        )
        .to_owned()
}

fn normalize_public_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

fn page_translation_response(translation: &page_translation::Model) -> PageTranslationResponse {
    PageTranslationResponse {
        locale: translation.locale.clone(),
        title: Some(translation.title.clone()),
        slug: Some(translation.slug.clone()),
        meta_title: translation.meta_title.clone(),
        meta_description: translation.meta_description.clone(),
    }
}

fn page_body_response(body: &page_body::Model) -> PageBodyResponse {
    let content_json =
        if body.format == CONTENT_FORMAT_RT_JSON_V1 || body.format == CONTENT_FORMAT_GRAPESJS_V1 {
            serde_json::from_str(&body.content).ok()
        } else {
            None
        };
    PageBodyResponse {
        locale: body.locale.clone(),
        content: body.content.clone(),
        format: body.format.clone(),
        content_json,
        updated_at: body.updated_at.to_string(),
    }
}

fn body_requires_json_payload(format: &str) -> bool {
    matches!(
        format,
        CONTENT_FORMAT_RT_JSON_V1 | CONTENT_FORMAT_GRAPESJS_V1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_channel_slugs_deduplicates_and_normalizes() {
        assert_eq!(
            normalize_channel_slugs(&[
                " Web ".to_string(),
                "mobile".to_string(),
                "web".to_string()
            ]),
            vec!["mobile".to_string(), "web".to_string()]
        );
    }

    #[test]
    fn page_visibility_respects_channel_allowlist() {
        let channel_slugs = vec!["web".to_string()];
        assert!(is_page_visible_for_channel(&channel_slugs, Some("web")));
        assert!(!is_page_visible_for_channel(&channel_slugs, Some("blog")));
        assert!(!is_page_visible_for_channel(&channel_slugs, None));
    }

    #[test]
    fn builder_publish_enabled_defaults_to_true() {
        assert!(is_builder_publish_enabled(&serde_json::json!({})));
        assert!(is_builder_publish_enabled(&serde_json::json!({
            "builder": {}
        })));
    }

    #[test]
    fn builder_publish_enabled_reads_nested_flag() {
        assert!(!is_builder_publish_enabled(&serde_json::json!({
            "builder": { "publish": { "enabled": false } }
        })));
        assert!(is_builder_publish_enabled(&serde_json::json!({
            "builder": { "publish": { "enabled": true } }
        })));
    }

    #[test]
    fn builder_enabled_defaults_to_true() {
        assert!(is_builder_enabled(&serde_json::json!({})));
        assert!(is_builder_enabled(&serde_json::json!({
            "builder": {}
        })));
    }

    #[test]
    fn builder_enabled_reads_top_level_flag() {
        assert!(!is_builder_enabled(&serde_json::json!({
            "builder": { "enabled": false }
        })));
        assert!(is_builder_enabled(&serde_json::json!({
            "builder": { "enabled": true }
        })));
    }

    #[test]
    fn builder_preview_enabled_defaults_to_true() {
        assert!(is_builder_preview_enabled(&serde_json::json!({})));
        assert!(is_builder_preview_enabled(&serde_json::json!({
            "builder": {}
        })));
    }

    #[test]
    fn builder_preview_enabled_reads_nested_flag() {
        assert!(!is_builder_preview_enabled(&serde_json::json!({
            "builder": { "preview": { "enabled": false } }
        })));
        assert!(is_builder_preview_enabled(&serde_json::json!({
            "builder": { "preview": { "enabled": true } }
        })));
    }

    #[test]
    fn builder_properties_enabled_defaults_to_true() {
        assert!(is_builder_properties_enabled(&serde_json::json!({})));
        assert!(is_builder_properties_enabled(&serde_json::json!({
            "builder": {}
        })));
    }

    #[test]
    fn builder_properties_enabled_reads_nested_flag() {
        assert!(!is_builder_properties_enabled(&serde_json::json!({
            "builder": { "properties": { "enabled": false } }
        })));
        assert!(is_builder_properties_enabled(&serde_json::json!({
            "builder": { "properties": { "enabled": true } }
        })));
    }
}
