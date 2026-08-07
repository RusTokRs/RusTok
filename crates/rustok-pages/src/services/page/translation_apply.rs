use chrono::Utc;
use rustok_api::TenantLocale;
use rustok_events::DomainEvent;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, sea_query::Expr,
};
use uuid::Uuid;

use crate::{
    entities::{page, page_translation},
    error::{PagesError, PagesResult},
    translation_evidence::{TranslationChangeEvidence, record_translation_change_in_tx},
};

use super::{
    PAGE_KIND, PageService,
    helpers::{
        next_page_translation_revision, next_page_version, normalize_slug, page_resource_revision,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplyExactPageMetadataTranslationInput {
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub title: String,
    pub slug: String,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
    pub expected_resource_revision: i64,
    pub expected_source_revision: i64,
    pub expected_target_revision: Option<i64>,
    pub actor_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PageMetadataTranslationApplyResult {
    pub resource_revision: i64,
    pub target_revision: i64,
}

impl PageService {
    pub(crate) async fn apply_exact_metadata_translation_in_tx(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
        input: ApplyExactPageMetadataTranslationInput,
    ) -> PagesResult<PageMetadataTranslationApplyResult> {
        if input.source_locale == input.target_locale {
            return Err(PagesError::validation(
                "Source and target locales must differ for an exact Page metadata translation",
            ));
        }
        if input.title.trim().is_empty() {
            return Err(PagesError::validation("Page title cannot be empty"));
        }
        let slug = normalize_slug(&input.slug)?;

        let page = self.find_page_for_update(txn, tenant_id, page_id).await?;
        let current_resource_revision = page_resource_revision(&page)?;
        if current_resource_revision != input.expected_resource_revision {
            return Err(PagesError::translation_conflict(
                "Page resource revision does not match the translation proposal",
            ));
        }
        if page.status == "archived" {
            return Err(PagesError::validation(
                "Archived Page metadata cannot receive a translation apply",
            ));
        }

        let source = page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::PageId.eq(page_id))
            .filter(page_translation::Column::Locale.eq(input.source_locale.as_str()))
            .one(txn)
            .await?
            .ok_or_else(|| PagesError::validation("Exact source Page locale is not present"))?;
        if source.revision != input.expected_source_revision {
            return Err(PagesError::translation_conflict(
                "Source Page locale revision does not match the translation proposal",
            ));
        }

        self.ensure_slug_unique_in_tx(
            txn,
            tenant_id,
            input.target_locale.as_str(),
            &slug,
            Some(page_id),
        )
        .await?;

        let existing_target = page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::PageId.eq(page_id))
            .filter(page_translation::Column::Locale.eq(input.target_locale.as_str()))
            .one(txn)
            .await?;
        let target_revision = match existing_target {
            Some(target) => {
                if input.expected_target_revision != Some(target.revision) {
                    return Err(PagesError::translation_conflict(
                        "Target Page locale revision does not match the translation proposal",
                    ));
                }
                let revision = next_page_translation_revision(
                    page_id,
                    input.target_locale.as_str(),
                    target.revision,
                )?;
                let updated = page_translation::Entity::update_many()
                    .col_expr(
                        page_translation::Column::Title,
                        Expr::value(input.title.clone()),
                    )
                    .col_expr(page_translation::Column::Slug, Expr::value(slug.clone()))
                    .col_expr(
                        page_translation::Column::MetaTitle,
                        Expr::value(input.meta_title.clone()),
                    )
                    .col_expr(
                        page_translation::Column::MetaDescription,
                        Expr::value(input.meta_description.clone()),
                    )
                    .col_expr(page_translation::Column::Revision, Expr::value(revision))
                    .filter(page_translation::Column::Id.eq(target.id))
                    .filter(page_translation::Column::Revision.eq(target.revision))
                    .exec(txn)
                    .await?;
                if updated.rows_affected != 1 {
                    return Err(PagesError::translation_conflict(
                        "Target Page locale changed before translation apply could commit",
                    ));
                }
                revision
            }
            None => {
                if input.expected_target_revision.is_some() {
                    return Err(PagesError::translation_conflict(
                        "Translation proposal expected a target Page locale that does not exist",
                    ));
                }
                let inserted = page_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    page_id: Set(page_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(input.target_locale.as_str().to_string()),
                    title: Set(input.title.clone()),
                    slug: Set(slug.clone()),
                    meta_title: Set(input.meta_title.clone()),
                    meta_description: Set(input.meta_description.clone()),
                    revision: Set(1),
                }
                .insert(txn)
                .await;
                match inserted {
                    Ok(_) => 1,
                    Err(error) if is_unique_constraint(&error) => {
                        return Err(PagesError::translation_conflict(
                            "Target Page locale was created before translation apply could commit",
                        ));
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        };

        let resource_version = next_page_version(page_id, page.version)?;
        let updated = page::Entity::update_many()
            .col_expr(page::Column::Version, Expr::value(resource_version))
            .col_expr(
                page::Column::UpdatedAt,
                Expr::value(Utc::now().fixed_offset()),
            )
            .filter(page::Column::Id.eq(page_id))
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Version.eq(page.version))
            .exec(txn)
            .await?;
        if updated.rows_affected != 1 {
            return Err(PagesError::translation_conflict(
                "Page changed before translation apply could commit",
            ));
        }

        let resource_revision = i64::from(resource_version);
        record_translation_change_in_tx(
            txn,
            TranslationChangeEvidence {
                tenant_id,
                page_id,
                resource_revision,
                operation: "upsert",
                lifecycle: "active",
            },
        )
        .await?;
        self.event_bus
            .publish_in_tx(
                txn,
                tenant_id,
                input.actor_id,
                DomainEvent::NodeUpdated {
                    node_id: page_id,
                    kind: PAGE_KIND.to_string(),
                },
            )
            .await?;

        Ok(PageMetadataTranslationApplyResult {
            resource_revision,
            target_revision,
        })
    }
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
