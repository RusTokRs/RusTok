use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
    TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_content::entities::node::ContentStatus;
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;

use crate::dto::{CreatePageInput, PageBodyRevisionSource, PageResponse, PageTranslationInput};
use crate::entities::{page, page_body, page_channel_visibility, page_translation};
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::enforce_scope;
use crate::translation_evidence::{TranslationChangeEvidence, record_translation_change_in_tx};

use super::helpers::{
    body_revision_timestamp, body_uses_builder_capability, build_page_metadata,
    normalize_channel_slugs, normalize_locale, normalize_page_body_input, normalize_slug,
    status_to_storage, validate_page_translations,
};
use super::{PAGE_KIND, PageService, PreparedPageBody};

/// Largest `-copy-N` suffix counter tried when allocating duplicated page slugs.
const MAX_DUPLICATE_SLUG_ATTEMPTS: usize = 50;

impl PageService {
    #[instrument(skip(self, input))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreatePageInput,
    ) -> PagesResult<PageResponse> {
        enforce_scope(&security, Resource::Pages, Action::Create)?;
        if input.publish {
            return Err(PagesError::validation(
                "Page creation cannot publish a Page Builder document; create the draft, review a runtime scenario, then use the atomic publish command",
            ));
        }
        validate_page_translations(&input.translations)?;
        let response_locale = normalize_locale(
            &input
                .translations
                .first()
                .expect("validated translations are non-empty")
                .locale,
        )?;
        let template = input
            .template
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let metadata = build_page_metadata(&template, None);
        let channel_slugs = normalize_channel_slugs(input.channel_slugs.as_deref().unwrap_or(&[]));
        let body = normalize_page_body_input(input.body)?;
        if let Some(body) = body.as_ref() {
            let has_translation = input.translations.iter().any(|translation| {
                normalize_locale(&translation.locale).is_ok_and(|locale| locale == body.locale)
            });
            if !has_translation {
                return Err(PagesError::validation(format!(
                    "Page document locale `{}` requires a matching page translation",
                    body.locale
                )));
            }
        }
        let now = body_revision_timestamp(Utc::now());
        let page_id = Uuid::new_v4();
        let txn = self.db.begin().await?;

        if body_uses_builder_capability(body.as_ref()) {
            Self::ensure_builder_enabled_in_tx(&txn, tenant_id).await?;
        }

        for translation in &input.translations {
            let slug = normalize_slug(
                translation
                    .slug
                    .as_deref()
                    .unwrap_or(translation.title.as_str()),
            )?;
            self.ensure_slug_unique_in_tx(&txn, tenant_id, &translation.locale, &slug, None)
                .await?;
        }

        let created_page = page::ActiveModel {
            id: Set(page_id),
            tenant_id: Set(tenant_id),
            author_id: Set(security.user_id),
            status: Set(status_to_storage(&ContentStatus::Draft).to_string()),
            template: Set(template),
            metadata: Set(metadata),
            created_at: Set(now),
            updated_at: Set(now),
            published_at: Set(None),
            archived_at: Set(None),
            version: Set(1),
        }
        .insert(&txn)
        .await?;

        self.replace_translations_in_tx(&txn, tenant_id, page_id, &input.translations)
            .await?;
        self.replace_channel_visibility_in_tx(&txn, tenant_id, page_id, &channel_slugs)
            .await?;
        self.upsert_body_in_tx(&txn, tenant_id, page_id, body.clone(), now)
            .await?;
        if let Some(body) = body.as_ref() {
            // A newly created page normally has no symbol catalog. Only sync
            // when it actually authors definitions; otherwise its empty seed
            // must not erase site-wide definitions from other pages.
            if !super::symbols::symbol_values_in_content(&body.content)?.is_empty() {
                super::symbols::sync_site_symbols_in_tx(
                    &txn,
                    tenant_id,
                    page_id,
                    &body.locale,
                    &body.content,
                )
                .await?;
            }
            let body_revision = now.to_string();
            self.record_body_revision_in_tx(
                &txn,
                tenant_id,
                page_id,
                &body.locale,
                &body.content,
                &body.format,
                PageBodyRevisionSource::Create,
                &body_revision,
                security.user_id,
            )
            .await?;
        }

        record_translation_change_in_tx(
            &txn,
            TranslationChangeEvidence {
                tenant_id,
                page_id,
                resource_revision: i64::from(created_page.version),
                operation: "upsert",
                lifecycle: "active",
            },
        )
        .await?;

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
        self.get_with_locale_fallback(tenant_id, security, page_id, &response_locale, None)
            .await
    }

    /// Copies one page into a new draft page.
    ///
    /// Translations keep their titles and metadata while slugs get a `-copy[-N]` suffix per
    /// locale, channel visibility is copied as-is, and every current body is copied into the
    /// new page with one `duplicate` body-revision journal row. Unpublished working copies of
    /// a published source are not copied.
    #[instrument(skip(self))]
    pub async fn duplicate_page(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PageResponse> {
        enforce_scope(&security, Resource::Pages, Action::Create)?;
        // Early existence check keeps the not-found contract stable before any write txn.
        let _ = self.find_page(tenant_id, page_id).await?;

        let now = body_revision_timestamp(Utc::now());
        let new_page_id = Uuid::new_v4();
        let txn = self.db.begin().await?;
        let source = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        let template = source.template.clone();
        let metadata = source.metadata.clone();

        let source_translations = page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::PageId.eq(page_id))
            .order_by_asc(page_translation::Column::Locale)
            .all(&txn)
            .await?;
        if source_translations.is_empty() {
            return Err(PagesError::validation(
                "Duplicating a page requires at least one source translation",
            ));
        }

        let mut translations = Vec::with_capacity(source_translations.len());
        for translation in &source_translations {
            let slug = self
                .allocate_duplicate_slug_in_tx(
                    &txn,
                    tenant_id,
                    &translation.locale,
                    &translation.slug,
                )
                .await?;
            translations.push(PageTranslationInput {
                locale: translation.locale.clone(),
                title: translation.title.clone(),
                slug: Some(slug),
                meta_title: translation.meta_title.clone(),
                meta_description: translation.meta_description.clone(),
            });
        }

        let source_bodies = page_body::Entity::find()
            .filter(page_body::Column::TenantId.eq(tenant_id))
            .filter(page_body::Column::PageId.eq(page_id))
            .order_by_asc(page_body::Column::Locale)
            .all(&txn)
            .await?;
        let source_channel_slugs = page_channel_visibility::Entity::find()
            .filter(page_channel_visibility::Column::TenantId.eq(tenant_id))
            .filter(page_channel_visibility::Column::PageId.eq(page_id))
            .order_by_asc(page_channel_visibility::Column::ChannelSlug)
            .all(&txn)
            .await?
            .into_iter()
            .map(|record| record.channel_slug)
            .collect::<Vec<_>>();

        let response_locale = normalize_locale(&translations[0].locale)?;
        let created_page = page::ActiveModel {
            id: Set(new_page_id),
            tenant_id: Set(tenant_id),
            author_id: Set(security.user_id),
            status: Set(status_to_storage(&ContentStatus::Draft).to_string()),
            template: Set(template),
            metadata: Set(metadata),
            created_at: Set(now),
            updated_at: Set(now),
            published_at: Set(None),
            archived_at: Set(None),
            version: Set(1),
        }
        .insert(&txn)
        .await?;

        self.replace_translations_in_tx(&txn, tenant_id, new_page_id, &translations)
            .await?;
        self.replace_channel_visibility_in_tx(&txn, tenant_id, new_page_id, &source_channel_slugs)
            .await?;

        let body_revision = now.to_string();
        for body in &source_bodies {
            let prepared = PreparedPageBody {
                locale: body.locale.clone(),
                content: body.content.clone(),
                format: body.format.clone(),
            };
            self.upsert_body_in_tx(&txn, tenant_id, new_page_id, Some(prepared.clone()), now)
                .await?;
            self.record_body_revision_in_tx(
                &txn,
                tenant_id,
                new_page_id,
                &prepared.locale,
                &prepared.content,
                &prepared.format,
                PageBodyRevisionSource::Duplicate,
                &body_revision,
                security.user_id,
            )
            .await?;
        }

        record_translation_change_in_tx(
            &txn,
            TranslationChangeEvidence {
                tenant_id,
                page_id: new_page_id,
                resource_revision: i64::from(created_page.version),
                operation: "upsert",
                lifecycle: "active",
            },
        )
        .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::NodeCreated {
                    node_id: new_page_id,
                    kind: PAGE_KIND.to_string(),
                    author_id: security.user_id,
                },
            )
            .await?;

        txn.commit().await?;
        self.get_with_locale_fallback(tenant_id, security, new_page_id, &response_locale, None)
            .await
    }

    async fn allocate_duplicate_slug_in_tx(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        source_slug: &str,
    ) -> PagesResult<String> {
        let base = normalize_slug(source_slug)?;
        let mut candidate = format!("{base}-copy");
        for attempt in 2..=MAX_DUPLICATE_SLUG_ATTEMPTS {
            match self
                .ensure_slug_unique_in_tx(txn, tenant_id, locale, &candidate, None)
                .await
            {
                Ok(()) => return Ok(candidate),
                Err(PagesError::DuplicateSlug { .. }) => {
                    candidate = format!("{base}-copy-{attempt}");
                }
                Err(error) => return Err(error),
            }
        }
        Err(PagesError::validation(format!(
            "Unable to allocate a unique `{locale}` slug for the duplicated page from `{base}`"
        )))
    }
}
