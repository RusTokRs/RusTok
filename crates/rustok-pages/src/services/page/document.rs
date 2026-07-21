use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait,
    QueryFilter, QuerySelect, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource};
use rustok_core::{CONTENT_FORMAT_GRAPESJS, SecurityContext, error::ErrorKind, error::RichError};
use rustok_events::DomainEvent;

use crate::dto::{PageResponse, SavePageDocumentInput};
use crate::entities::{page, page_body, page_translation};
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::enforce_owned_scope;

use super::helpers::{body_uses_builder_capability, normalize_page_body_input};
use super::{PAGE_KIND, PageService};

pub const PAGE_DOCUMENT_REVISION_CONFLICT: &str = "PAGE_DOCUMENT_REVISION_CONFLICT";
pub const PAGE_PUBLISHED_DOCUMENT_IMMUTABLE: &str = "PAGE_PUBLISHED_DOCUMENT_IMMUTABLE";

impl PageService {
    #[instrument(skip(self, input))]
    pub async fn save_document(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: SavePageDocumentInput,
    ) -> PagesResult<PageResponse> {
        let body = normalize_page_body_input(Some(input.body))?
            .ok_or_else(|| PagesError::validation("Page document body is required"))?;
        if body.format != CONTENT_FORMAT_GRAPESJS {
            return Err(PagesError::validation(
                "Page document save accepts only the current Fly/GrapesJS body format",
            ));
        }
        if body_uses_builder_capability(Some(&body)) {
            self.ensure_builder_enabled(tenant_id).await?;
        }

        let observed = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            observed.author_id,
        )?;
        ensure_document_is_mutable(&observed)?;

        let response_locale = body.locale.clone();
        let txn = self.db.begin().await?;
        let locked_page = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            locked_page.author_id,
        )?;
        ensure_document_is_mutable(&locked_page)?;

        let translation_exists = page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::PageId.eq(page_id))
            .filter(page_translation::Column::Locale.eq(&body.locale))
            .one(&txn)
            .await?
            .is_some();
        if !translation_exists {
            return Err(PagesError::validation(format!(
                "Page document locale `{}` requires a matching page translation",
                body.locale
            )));
        }

        let body_query = || {
            page_body::Entity::find()
                .filter(page_body::Column::TenantId.eq(tenant_id))
                .filter(page_body::Column::PageId.eq(page_id))
                .filter(page_body::Column::Locale.eq(&body.locale))
        };
        let existing = match txn.get_database_backend() {
            DbBackend::Sqlite => body_query().one(&txn).await?,
            DbBackend::Postgres | DbBackend::MySql => {
                body_query().lock_exclusive().one(&txn).await?
            }
        };
        let actual_revision = page_document_revision(page_id, existing.as_ref());
        if input.expected_revision != actual_revision {
            return Err(document_revision_conflict(
                input.expected_revision,
                actual_revision,
            ));
        }

        let now = Utc::now();
        match existing {
            Some(existing) => {
                let mut active: page_body::ActiveModel = existing.into();
                active.content = Set(body.content);
                active.format = Set(body.format);
                active.updated_at = Set(now.into());
                active.update(&txn).await?;
            }
            None => {
                page_body::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    page_id: Set(page_id),
                    locale: Set(body.locale),
                    content: Set(body.content),
                    format: Set(body.format),
                    updated_at: Set(now.into()),
                }
                .insert(&txn)
                .await?;
            }
        }

        let mut page_active: page::ActiveModel = locked_page.into();
        page_active.updated_at = Set(now.into());
        page_active.update(&txn).await?;

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

pub(crate) fn page_document_revision(page_id: Uuid, body: Option<&page_body::Model>) -> String {
    body.map(|body| body.updated_at.to_string())
        .unwrap_or_else(|| format!("page:{page_id}:initial"))
}

fn ensure_document_is_mutable(page: &page::Model) -> PagesResult<()> {
    if page.status == "published" {
        return Err(PagesError::Rich(Box::new(
            RichError::new(
                ErrorKind::Conflict,
                "Published page documents are immutable without a separate draft revision",
            )
            .with_user_message(
                "Unpublish this page before editing its visual document, then publish the new revision explicitly.",
            )
            .with_field("page_id", page.id.to_string())
            .with_error_code(PAGE_PUBLISHED_DOCUMENT_IMMUTABLE),
        )));
    }
    Ok(())
}

pub(super) fn document_revision_conflict(expected: String, actual: String) -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(
            ErrorKind::Conflict,
            format!(
                "Page document changed concurrently: expected revision `{expected}`, found `{actual}`"
            ),
        )
        .with_user_message(
            "The visual document changed while you were editing it. Reload the latest document and retry.",
        )
        .with_field("expected_revision", expected)
        .with_field("actual_revision", actual)
        .with_error_code(PAGE_DOCUMENT_REVISION_CONFLICT),
    ))
}
