use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, TransactionTrait};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_content::entities::node::ContentStatus;
use rustok_core::{CONTENT_FORMAT_GRAPESJS, SecurityContext};
use rustok_events::DomainEvent;

use crate::dto::{CreatePageInput, PageResponse};
use crate::entities::page;
use crate::error::PagesResult;
use crate::services::PageBuilderArtifactService;
use crate::services::rbac::enforce_scope;

use super::helpers::{
    body_uses_builder_capability, build_page_metadata, normalize_channel_slugs,
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
}
