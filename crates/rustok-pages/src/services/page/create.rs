use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, TransactionTrait};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_content::entities::node::ContentStatus;
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;

use crate::dto::{CreatePageInput, PageResponse};
use crate::entities::page;
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::enforce_scope;

use super::helpers::{
    body_uses_builder_capability, build_page_metadata, normalize_channel_slugs, normalize_locale,
    normalize_page_body_input, normalize_slug, status_to_storage, validate_page_translations,
};
use super::{PAGE_KIND, PageService};

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
                normalize_locale(&translation.locale)
                    .is_ok_and(|locale| locale == body.locale)
            });
            if !has_translation {
                return Err(PagesError::validation(format!(
                    "Page document locale `{}` requires a matching page translation",
                    body.locale
                )));
            }
        }
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
            )?;
            self.ensure_slug_unique_in_tx(&txn, tenant_id, &translation.locale, &slug, None)
                .await?;
        }

        page::ActiveModel {
            id: Set(page_id),
            tenant_id: Set(tenant_id),
            author_id: Set(security.user_id),
            status: Set(status_to_storage(&ContentStatus::Draft).to_string()),
            template: Set(template),
            metadata: Set(metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
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
        self.upsert_body_in_tx(&txn, tenant_id, page_id, body, now)
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
}
