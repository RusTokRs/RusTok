use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, TransactionTrait};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource};
use rustok_content::entities::node::ContentStatus;
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;

use crate::dto::{PageResponse, PatchPageMetadataInput};
use crate::entities::page;
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::{enforce_owned_scope, enforce_scope};

use super::helpers::{
    build_page_metadata, enforce_expected_version, normalize_channel_slugs, normalize_slug,
    storage_to_status, validate_page_translations,
};
use super::{PAGE_KIND, PageService};

impl PageService {
    #[instrument(skip(self, input))]
    pub async fn patch_metadata(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: PatchPageMetadataInput,
    ) -> PagesResult<PageResponse> {
        if input.translations.is_none() && input.template.is_none() && input.channel_slugs.is_none()
        {
            return Err(PagesError::validation(
                "Page metadata patch must include translations, template or channel slugs",
            ));
        }
        if let Some(translations) = input.translations.as_deref() {
            validate_page_translations(translations)?;
        }

        let observed = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            observed.author_id,
        )?;
        enforce_expected_version(Some(input.expected_version), observed.version)?;
        if storage_to_status(&observed.status)? == ContentStatus::Published {
            enforce_scope(&security, Resource::Pages, Action::Publish)?;
        }

        let template = input
            .template
            .clone()
            .unwrap_or_else(|| observed.template.clone());
        let metadata = build_page_metadata(&template, Some(&observed.metadata));
        let channel_slugs = input
            .channel_slugs
            .as_ref()
            .map(|items| normalize_channel_slugs(items));
        let response_locale = input
            .translations
            .as_ref()
            .and_then(|items| items.first().map(|item| item.locale.clone()))
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());

        let txn = self.db.begin().await?;
        let locked = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        enforce_expected_version(Some(input.expected_version), locked.version)?;
        enforce_owned_scope(&security, Resource::Pages, Action::Update, locked.author_id)?;

        for translation in input.translations.as_deref().unwrap_or(&[]) {
            let slug = normalize_slug(
                translation
                    .slug
                    .as_deref()
                    .unwrap_or(translation.title.as_str()),
            )?;
            self.ensure_slug_unique_in_tx(
                &txn,
                tenant_id,
                &translation.locale,
                &slug,
                Some(page_id),
            )
            .await?;
        }

        let now = Utc::now();
        let mut active: page::ActiveModel = locked.into();
        active.template = Set(template);
        active.metadata = Set(metadata);
        active.updated_at = Set(now.into());
        active.version = Set(active.version.take().unwrap_or(1) + 1);
        active.update(&txn).await?;

        if let Some(translations) = input.translations.as_deref() {
            self.replace_translations_in_tx(&txn, tenant_id, page_id, translations)
                .await?;
        }
        if let Some(channel_slugs) = channel_slugs.as_deref() {
            self.replace_channel_visibility_in_tx(&txn, tenant_id, page_id, channel_slugs)
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
}
