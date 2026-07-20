use chrono::Utc;
use sea_orm::{
    sea_query::{Expr, Query, SelectStatement},
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Select, TransactionTrait,
};
use std::collections::{BTreeMap, HashMap};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource, PLATFORM_FALLBACK_LOCALE};
use rustok_content::{
    available_locales_from, entities::node::ContentStatus, normalize_locale_code,
    resolve_by_locale_with_fallback,
};
use rustok_core::{
    normalize_content_format, prepare_content_payload, SecurityContext, CONTENT_FORMAT_GRAPESJS,
    CONTENT_FORMAT_RT_JSON_V1,
};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::*;
use crate::entities::{page, page_body, page_channel_visibility, page_translation};
use crate::error::{
    PagesError, PagesResult, FEATURE_BUILDER_ENABLED, FEATURE_BUILDER_PREVIEW_ENABLED,
    FEATURE_BUILDER_PROPERTIES_ENABLED, FEATURE_BUILDER_PUBLISH_ENABLED,
};
use crate::services::page_builder_artifact::CompiledLandingArtifact;
use crate::services::rbac::{can_read_non_public_pages, enforce_owned_scope, enforce_scope};
use crate::services::{
    BlockService, PageBuilderArtifactService, PageBuilderScenarioBaselineService,
};
use rustok_tenant::TenantService;

const PAGE_KIND: &str = "page";
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageTransition {
    Publish,
    Unpublish,
    Archive,
}

impl PageTransition {
    fn from_status(status: Option<&ContentStatus>) -> Option<Self> {
        match status {
            Some(ContentStatus::Published) => Some(Self::Publish),
            Some(ContentStatus::Draft) => Some(Self::Unpublish),
            Some(ContentStatus::Archived) => Some(Self::Archive),
            None => None,
        }
    }

    fn status(self) -> ContentStatus {
        match self {
            Self::Publish => ContentStatus::Published,
            Self::Unpublish => ContentStatus::Draft,
            Self::Archive => ContentStatus::Archived,
        }
    }
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
        }
        validate_page_translations(&input.translations)?;
        let template = input
            .template
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let metadata = build_page_metadata(&template, &input.translations, None);
        let channel_slugs = normalize_channel_slugs(input.channel_slugs.as_deref().unwrap_or(&[]));
        let body = normalize_page_body_input(input.body)?;
        let builder_body = body_uses_builder_capability(body.as_ref());
        if builder_body {
            self.ensure_builder_enabled(tenant_id).await?;
            if input.publish {
                self.ensure_builder_publish_enabled(tenant_id).await?;
            }
        }
        let compiled = if input.publish {
            body.as_ref()
                .filter(|body| body.format == CONTENT_FORMAT_GRAPESJS)
                .map(|body| PageBuilderArtifactService::compile_source(&body.locale, &body.content))
                .transpose()?
        } else {
            None
        };
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
            ContentStatus::Published
        } else {
            ContentStatus::Draft
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
            published_at: Set(input.publish.then(|| now.into())),
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
        if let Some(compiled) = compiled.as_ref() {
            let artifact_id = PageBuilderArtifactService::stage_compiled_in_tx(
                &txn, tenant_id, page_id, compiled,
            )
            .await?;
            PageBuilderArtifactService::bind_existing_body_in_tx(
                &txn,
                tenant_id,
                page_id,
                &compiled.locale,
                artifact_id,
            )
            .await?;
        }
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
        if input.publish {
            self.event_bus
                .publish_in_tx(
                    &txn,
                    tenant_id,
                    security.user_id,
                    DomainEvent::NodePublished {
                        node_id: page_id,
                        kind: PAGE_KIND.to_string(),
                    },
                )
                .await?;
        }

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
        if let Some(ref translations) = input.translations {
            validate_page_translations(translations)?;
        }

        let observed = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            observed.author_id,
        )?;
        enforce_expected_version(input.expected_version, observed.version)?;

        let transition = PageTransition::from_status(input.status.as_ref());
        let current_status = storage_to_status(&observed.status)?;
        let body = normalize_page_body_input(input.body)?;
        let candidate_uses_builder = body_uses_builder_capability(body.as_ref());
        let changes_page_content = input.translations.is_some()
            || input.template.is_some()
            || body.is_some()
            || input.channel_slugs.is_some();
        let mutates_public_page =
            current_status == ContentStatus::Published && changes_page_content;
        if transition.is_some() || mutates_public_page {
            enforce_scope(&security, Resource::Pages, Action::Publish)?;
        }
        if candidate_uses_builder {
            self.ensure_builder_enabled(tenant_id).await?;
        }

        let existing_bodies = if transition == Some(PageTransition::Publish) {
            self.load_bodies(page_id).await?
        } else {
            Vec::new()
        };
        let effective_builder_projects = if transition == Some(PageTransition::Publish) {
            collect_builder_project_values(&existing_bodies, body.as_ref(), true)?
        } else if transition.is_none()
            && current_status == ContentStatus::Published
            && candidate_uses_builder
        {
            collect_builder_project_values(&[], body.as_ref(), false)?
        } else {
            Vec::new()
        };
        if !effective_builder_projects.is_empty() {
            self.ensure_builder_enabled(tenant_id).await?;
            self.ensure_builder_publish_enabled(tenant_id).await?;
            PageBuilderScenarioBaselineService::new(self.db.clone())
                .ensure_candidates_allowed(tenant_id, page_id, effective_builder_projects)
                .await?;
        }

        let compiled = if transition == Some(PageTransition::Publish) {
            compile_builder_sources(&existing_bodies, body.as_ref(), true)?
        } else if transition.is_none()
            && current_status == ContentStatus::Published
            && candidate_uses_builder
        {
            compile_builder_sources(&[], body.as_ref(), false)?
        } else {
            Vec::new()
        };

        let template = input
            .template
            .clone()
            .unwrap_or_else(|| observed.template.clone());
        let metadata = build_page_metadata(
            &template,
            input.translations.as_deref().unwrap_or(&[]),
            Some(&observed.metadata),
        );
        let channel_slugs = input
            .channel_slugs
            .as_ref()
            .map(|items| normalize_channel_slugs(items))
            .unwrap_or_default();
        let replace_channel_visibility = input.channel_slugs.is_some();
        let response_locale = body
            .as_ref()
            .map(|body| body.locale.clone())
            .or_else(|| {
                input
                    .translations
                    .as_ref()
                    .and_then(|items| items.first().map(|item| item.locale.clone()))
            })
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());

        let txn = self.db.begin().await?;
        let locked = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        enforce_expected_version(Some(observed.version), locked.version)?;
        enforce_owned_scope(&security, Resource::Pages, Action::Update, locked.author_id)?;

        for translation in input.translations.as_deref().unwrap_or(&[]) {
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

        let mut staged_artifacts = Vec::with_capacity(compiled.len());
        for compiled in &compiled {
            let artifact_id = PageBuilderArtifactService::stage_compiled_in_tx(
                &txn, tenant_id, page_id, compiled,
            )
            .await?;
            staged_artifacts.push((compiled.locale.clone(), artifact_id));
        }

        let now = Utc::now();
        let mut active: page::ActiveModel = locked.into();
        active.template = Set(template);
        active.metadata = Set(metadata);
        active.updated_at = Set(now.into());
        active.version = Set(active.version.take().unwrap_or(1) + 1);
        apply_transition(&mut active, transition, now);
        active.update(&txn).await?;

        if let Some(ref translations) = input.translations {
            self.replace_translations_in_tx(&txn, tenant_id, page_id, translations)
                .await?;
        }
        if replace_channel_visibility {
            self.replace_channel_visibility_in_tx(&txn, tenant_id, page_id, &channel_slugs)
                .await?;
        }
        if let Some(body) = body
            .as_ref()
            .filter(|body| body.format != CONTENT_FORMAT_GRAPESJS)
        {
            PageBuilderArtifactService::clear_existing_body_binding_in_tx(
                &txn,
                page_id,
                &body.locale,
            )
            .await?;
        }
        self.upsert_body_in_tx(&txn, page_id, body, now).await?;
        for (locale, artifact_id) in staged_artifacts {
            PageBuilderArtifactService::bind_existing_body_in_tx(
                &txn,
                tenant_id,
                page_id,
                &locale,
                artifact_id,
            )
            .await?;
        }

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
        if let Some(event) = transition_event(transition, page_id) {
            self.event_bus
                .publish_in_tx(&txn, tenant_id, security.user_id, event)
                .await?;
        }
        txn.commit().await?;
        self.get_with_locale_fallback(
            tenant_id,
            security,
            page_id,
            &response_locale,
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
        self.publish_if_current(tenant_id, security, page_id, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn publish_if_current(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
    ) -> PagesResult<PageResponse> {
        let observed = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            observed.author_id,
        )?;
        enforce_expected_version(expected_version, observed.version)?;
        let bodies = self.load_bodies(page_id).await?;
        let project_values = collect_builder_project_values(&bodies, None, true)?;
        if !project_values.is_empty() {
            self.ensure_builder_enabled(tenant_id).await?;
            self.ensure_builder_publish_enabled(tenant_id).await?;
            PageBuilderScenarioBaselineService::new(self.db.clone())
                .ensure_candidates_allowed(tenant_id, page_id, project_values)
                .await?;
        }
        let compiled = compile_builder_sources(&bodies, None, true)?;
        self.transition_page(
            tenant_id,
            security,
            page_id,
            PageTransition::Publish,
            observed.version,
            &compiled,
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
        self.unpublish_if_current(tenant_id, security, page_id, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn unpublish_if_current(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
    ) -> PagesResult<PageResponse> {
        let observed = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            observed.author_id,
        )?;
        enforce_expected_version(expected_version, observed.version)?;
        self.transition_page(
            tenant_id,
            security,
            page_id,
            PageTransition::Unpublish,
            observed.version,
            &[],
        )
        .await
    }

    #[instrument(skip(self))]
    pub async fn ensure_builder_preview_enabled_for_tenant(
        &self,
        tenant_id: Uuid,
    ) -> PagesResult<()> {
        let module = self.load_tenant_pages_module(tenant_id).await?;
        let enabled = module
            .as_ref()
            .map(is_builder_preview_enabled)
            .unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(
                FEATURE_BUILDER_PREVIEW_ENABLED,
            ));
        }
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn ensure_builder_properties_enabled_for_tenant(
        &self,
        tenant_id: Uuid,
    ) -> PagesResult<()> {
        let module = self.load_tenant_pages_module(tenant_id).await?;
        let enabled = module
            .as_ref()
            .map(is_builder_properties_enabled)
            .unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(
                FEATURE_BUILDER_PROPERTIES_ENABLED,
            ));
        }
        Ok(())
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

    async fn transition_page(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        transition: PageTransition,
        expected_version: i32,
        compiled: &[CompiledLandingArtifact],
    ) -> PagesResult<PageResponse> {
        let txn = self.db.begin().await?;
        let existing = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        enforce_expected_version(Some(expected_version), existing.version)?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            existing.author_id,
        )?;

        if transition == PageTransition::Publish {
            for compiled in compiled {
                let artifact_id = PageBuilderArtifactService::stage_compiled_in_tx(
                    &txn, tenant_id, page_id, compiled,
                )
                .await?;
                PageBuilderArtifactService::bind_existing_body_in_tx(
                    &txn,
                    tenant_id,
                    page_id,
                    &compiled.locale,
                    artifact_id,
                )
                .await?;
            }
        }

        let now = Utc::now();
        let mut active: page::ActiveModel = existing.into();
        active.updated_at = Set(now.into());
        active.version = Set(active.version.take().unwrap_or(1) + 1);
        apply_transition(&mut active, Some(transition), now);
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
        if let Some(event) = transition_event(Some(transition), page_id) {
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
            .map(is_builder_publish_enabled)
            .unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(
                FEATURE_BUILDER_PUBLISH_ENABLED,
            ));
        }
        Ok(())
    }

    async fn ensure_builder_enabled(&self, tenant_id: Uuid) -> PagesResult<()> {
        let module = self.load_tenant_pages_module(tenant_id).await?;
        let enabled = module.as_ref().map(is_builder_enabled).unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(FEATURE_BUILDER_ENABLED));
        }
        Ok(())
    }

    async fn load_tenant_pages_module(
        &self,
        tenant_id: Uuid,
    ) -> PagesResult<Option<serde_json::Value>> {
        TenantService::new(self.db.clone())
            .find_tenant_module(tenant_id, "pages")
            .await
            .map(|module| module.map(|module| module.settings))
            .map_err(Into::into)
    }

    async fn find_page_for_update(
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
            version: page.version,
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
    let locale = normalize_locale(&body.locale)?;
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
        &locale,
        "Body",
    )
    .map_err(PagesError::validation)?;
    Ok(Some(PreparedPageBody {
        locale,
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
    body.is_some_and(|item| item.format == CONTENT_FORMAT_GRAPESJS)
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

fn collect_builder_project_values(
    existing_bodies: &[page_body::Model],
    candidate: Option<&PreparedPageBody>,
    include_existing: bool,
) -> PagesResult<Vec<serde_json::Value>> {
    collect_builder_sources(existing_bodies, candidate, include_existing)
        .into_iter()
        .map(|(locale, content)| {
            serde_json::from_str(&content).map_err(|error| {
                PagesError::validation(format!(
                    "Page Builder project for locale `{locale}` is not valid JSON: {error}"
                ))
            })
        })
        .collect()
}

fn collect_builder_sources(
    existing_bodies: &[page_body::Model],
    candidate: Option<&PreparedPageBody>,
    include_existing: bool,
) -> BTreeMap<String, String> {
    let mut sources = BTreeMap::<String, String>::new();
    if include_existing {
        for body in existing_bodies {
            if body.format == CONTENT_FORMAT_GRAPESJS {
                sources.insert(body.locale.clone(), body.content.clone());
            }
        }
    }
    if let Some(candidate) = candidate {
        if candidate.format == CONTENT_FORMAT_GRAPESJS {
            sources.insert(candidate.locale.clone(), candidate.content.clone());
        } else {
            sources.remove(&candidate.locale);
        }
    }
    sources
}

fn compile_builder_sources(
    existing_bodies: &[page_body::Model],
    candidate: Option<&PreparedPageBody>,
    include_existing: bool,
) -> PagesResult<Vec<CompiledLandingArtifact>> {
    collect_builder_sources(existing_bodies, candidate, include_existing)
        .into_iter()
        .map(|(locale, content)| PageBuilderArtifactService::compile_source(&locale, &content))
        .collect()
}

fn enforce_expected_version(expected: Option<i32>, actual: i32) -> PagesResult<()> {
    if let Some(expected_version) = expected {
        if expected_version != actual {
            return Err(PagesError::VersionConflict {
                expected_version,
                actual_version: actual,
            });
        }
    }
    Ok(())
}

fn apply_transition(
    active: &mut page::ActiveModel,
    transition: Option<PageTransition>,
    now: chrono::DateTime<Utc>,
) {
    let Some(transition) = transition else {
        return;
    };
    active.status = Set(status_to_storage(&transition.status()).to_string());
    match transition {
        PageTransition::Publish => {
            active.published_at = Set(Some(now.into()));
            active.archived_at = Set(None);
        }
        PageTransition::Unpublish => {
            active.published_at = Set(None);
            active.archived_at = Set(None);
        }
        PageTransition::Archive => {
            active.published_at = Set(None);
            active.archived_at = Set(Some(now.into()));
        }
    }
}

fn transition_event(transition: Option<PageTransition>, page_id: Uuid) -> Option<DomainEvent> {
    match transition {
        Some(PageTransition::Publish) => Some(DomainEvent::NodePublished {
            node_id: page_id,
            kind: PAGE_KIND.to_string(),
        }),
        Some(PageTransition::Unpublish) => Some(DomainEvent::NodeUnpublished {
            node_id: page_id,
            kind: PAGE_KIND.to_string(),
        }),
        Some(PageTransition::Archive) | None => None,
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
        if body.format == CONTENT_FORMAT_RT_JSON_V1 || body.format == CONTENT_FORMAT_GRAPESJS {
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
    matches!(format, CONTENT_FORMAT_RT_JSON_V1 | CONTENT_FORMAT_GRAPESJS)
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
    fn expected_version_fails_closed_on_stale_writes() {
        assert!(enforce_expected_version(None, 4).is_ok());
        assert!(enforce_expected_version(Some(4), 4).is_ok());
        assert!(matches!(
            enforce_expected_version(Some(3), 4),
            Err(PagesError::VersionConflict {
                expected_version: 3,
                actual_version: 4,
            })
        ));
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

    #[test]
    fn builder_body_locale_is_normalized_before_source_collection() {
        let prepared = normalize_page_body_input(Some(PageBodyInput {
            locale: " EN ".to_string(),
            content: String::new(),
            format: Some(CONTENT_FORMAT_GRAPESJS.to_string()),
            content_json: Some(serde_json::json!({})),
        }))
        .expect("valid builder body")
        .expect("prepared body");

        assert_eq!(prepared.locale, "en");
        let sources = collect_builder_sources(&[], Some(&prepared), false);
        assert_eq!(sources.keys().cloned().collect::<Vec<_>>(), vec!["en"]);
    }
}
