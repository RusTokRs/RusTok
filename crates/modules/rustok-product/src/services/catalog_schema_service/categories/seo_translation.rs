use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource, TenantLocale};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_events::DomainEvent;
use rustok_outbox::idempotency::Admission;
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldPatch,
    TranslationFieldSnapshot, TranslationPatchRequest, TranslationPatchValidation,
    TranslationResourceIdentity, TranslationResourceLifecycle, TranslationResourcePage,
    TranslationResourceSnapshot, TranslationResourceSummary, TranslationStrategy,
    TranslationTargetCapability, TranslationTargetChange, TranslationTargetChangePage,
    TranslationTargetChangesRequest, TranslationTargetProgressFacts, TranslationTargetProgressRequest,
    TranslationTargetProvider, TranslationTargetProviderDescriptor, TranslationValueProfile,
    provider_support::{
        contract_validation_error, field_hash, merged_patch_values, normalize_optional_target_value,
        read_request_from_patch, validate_patch_against_snapshot, validation_to_port_error,
    },
    validate_translation_apply_context, validate_translation_read_context,
};
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseBackend, DatabaseTransaction, FromQueryResult,
    IsolationLevel, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::error::{CommerceError, CommerceResult};
use crate::services::write_transaction::{
    ProductWriteTransaction, current_product_operation_id, record_product_operation_result,
    with_product_operation_receipt,
};

use super::super::ProductCatalogSchemaService;

const TRANSLATION_OWNER_SLUG: &str = "product";
const TRANSLATION_RESOURCE_KIND: &str = "category_seo";
const META_TITLE_FIELD: &str = "meta_title";
const META_DESCRIPTION_FIELD: &str = "meta_description";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_category_seo_patch";
const CHANGE_CURSOR_VERSION: &str = "v1";
const MAX_CHANGE_PAGE: u16 = 200;
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;
const RESOURCE_REVISION_PREFIX: &str = "product-category-seo-resource-v1:";
const LOCALE_REVISION_NAMESPACE: &str = "rustok-product/category-seo-translation-locale/v1";
const DELETED_REVISION_NAMESPACE: &str = "rustok-product/category-seo-translation-deleted/v1";

#[derive(Debug, Error)]
enum ProductCategorySeoTranslationError {
    #[error(transparent)]
    Commerce(#[from] CommerceError),
    #[error("Product Category SEO resource not found: {0}")]
    CategorySeoNotFound(Uuid),
    #[error("Product Category SEO source locale not found: {locale} for category {category_id}")]
    SourceLocaleNotFound { category_id: Uuid, locale: String },
    #[error("Product Category SEO {revision} revision conflict")]
    RevisionConflict { revision: &'static str },
    #[error("Product Category SEO owner invariant failed: {0}")]
    OwnerInvariant(String),
}

impl From<sea_orm::DbErr> for ProductCategorySeoTranslationError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Commerce(CommerceError::Database(error))
    }
}

type OwnerResult<T> = Result<T, ProductCategorySeoTranslationError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
struct SeoCopy {
    meta_title: Option<String>,
    meta_description: Option<String>,
}

impl SeoCopy {
    fn has_copy(&self) -> bool {
        nonblank(self.meta_title.as_deref()) || nonblank(self.meta_description.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredTranslation {
    locale: String,
    meta_title: Option<String>,
    meta_description: Option<String>,
}

impl StoredTranslation {
    fn copy(&self) -> SeoCopy {
        SeoCopy {
            meta_title: self.meta_title.clone(),
            meta_description: self.meta_description.clone(),
        }
    }

    fn has_copy(&self) -> bool {
        self.copy().has_copy()
    }
}

#[derive(Debug, Clone)]
struct Aggregate {
    id: Uuid,
    tenant_id: Uuid,
    code: String,
    is_active: bool,
    deleted: bool,
    translation_revision: u64,
    translations: Vec<StoredTranslation>,
}

impl Aggregate {
    fn has_copy(&self) -> bool {
        self.translations.iter().any(StoredTranslation::has_copy)
    }
}

#[derive(Debug, FromQueryResult)]
struct AggregateRow {
    id: Uuid,
    tenant_id: Uuid,
    code: String,
    is_active: bool,
    deleted: bool,
    translation_revision: i64,
    translations: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExactLocaleSnapshot {
    category_id: Uuid,
    category_code: String,
    is_active: bool,
    source_locale: String,
    target_locale: String,
    resource_revision: String,
    source_revision: String,
    target_revision: Option<String>,
    exact_locales: Vec<String>,
    source: SeoCopy,
    target: Option<SeoCopy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExactLocaleApply {
    source_locale: String,
    target_locale: String,
    meta_title: Option<String>,
    meta_description: Option<String>,
    expected_resource_revision: String,
    expected_source_revision: String,
    expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExactLocaleApplyReceipt {
    operation_id: Option<Uuid>,
    category_id: Uuid,
    resource_revision: String,
    target_revision: String,
    target: SeoCopy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProgressFacts {
    resources: u64,
    optional_units: u64,
    exact_optional_units: u64,
    complete_resources: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangeLifecycle {
    Active,
    Archived,
    Deleted,
}

impl ChangeLifecycle {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Deleted => "deleted",
        }
    }

    fn parse(value: &str) -> OwnerResult<Self> {
        match value {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            "deleted" => Ok(Self::Deleted),
            _ => Err(ProductCategorySeoTranslationError::OwnerInvariant(
                "change journal returned an unknown lifecycle".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChangeRecord {
    change_seq: u64,
    category_id: Uuid,
    resource_revision: String,
    lifecycle: ChangeLifecycle,
}

#[derive(Debug, FromQueryResult)]
struct PreviousChangeRow {
    resource_revision: String,
    lifecycle: String,
}

#[derive(Clone)]
struct ProductCategorySeoTranslationTargetProvider {
    service: Arc<ProductCatalogSchemaService>,
}

impl ProductCatalogSchemaService {
    /// Builds the Product-owned Translation adapter for Category-local SEO copy.
    ///
    /// Canonical Category name/slug/description remain Taxonomy-owned. SEO-module overrides remain
    /// `seo/seo_copy`; this resource owns only Product's `catalog_category_seo_translations` rows.
    pub fn category_seo_translation_target_provider(
        service: Arc<Self>,
    ) -> impl TranslationTargetProvider {
        ProductCategorySeoTranslationTargetProvider { service }
    }

    async fn list_category_seo_translation_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<Uuid>,
        limit: u16,
    ) -> OwnerResult<(Vec<ExactLocaleSnapshot>, Option<Uuid>)> {
        validate_uuid(tenant_id, "tenant_id")?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        if limit == 0 || limit > 200 {
            return Err(CommerceError::Validation(
                "Product Category SEO translation resource page size must be between 1 and 200"
                    .to_string(),
            )
            .into());
        }
        ensure_postgres(self.db.get_database_backend())?;
        let mut aggregates = load_source_eligible_page(
            &self.db,
            tenant_id,
            &source_locale,
            after,
            limit.saturating_add(1),
        )
        .await?;
        let has_more = aggregates.len() > usize::from(limit);
        if has_more {
            aggregates.truncate(usize::from(limit));
        }
        let next_after = has_more.then(|| aggregates.last().map(|row| row.id)).flatten();
        let resources = aggregates
            .into_iter()
            .map(|aggregate| build_snapshot(aggregate, &source_locale, &target_locale))
            .collect::<OwnerResult<Vec<_>>>()?;
        Ok((resources, next_after))
    }

    async fn read_category_seo_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> OwnerResult<ExactLocaleSnapshot> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_uuid(category_id, "category_id")?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        ensure_postgres(self.db.get_database_backend())?;
        let aggregate = load_category_seo_aggregate(&self.db, tenant_id, category_id)
            .await?
            .filter(|aggregate| !aggregate.deleted && aggregate.has_copy())
            .ok_or(ProductCategorySeoTranslationError::CategorySeoNotFound(
                category_id,
            ))?;
        build_snapshot(aggregate, &source_locale, &target_locale)
    }

    async fn apply_category_seo_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        category_id: Uuid,
        request: ExactLocaleApply,
    ) -> OwnerResult<ExactLocaleApplyReceipt> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_uuid(category_id, "category_id")?;
        if let Some(actor_user_id) = actor_user_id {
            validate_uuid(actor_user_id, "actor_user_id")?;
        }
        let source_locale = canonical_locale(&request.source_locale)?;
        let target_locale = canonical_locale(&request.target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        validate_copy(&SeoCopy {
            meta_title: request.meta_title.clone(),
            meta_description: request.meta_description.clone(),
        })?;
        ensure_postgres(self.db.get_database_backend())?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        let locked = txn
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
SELECT id
FROM catalog_categories
WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
FOR UPDATE
"#,
                vec![tenant_id.into(), category_id.into()],
            ))
            .await?;
        if locked.is_none() {
            return Err(ProductCategorySeoTranslationError::CategorySeoNotFound(
                category_id,
            ));
        }

        let before = build_snapshot(
            load_category_seo_aggregate(&txn, tenant_id, category_id)
                .await?
                .filter(|aggregate| !aggregate.deleted && aggregate.has_copy())
                .ok_or(ProductCategorySeoTranslationError::CategorySeoNotFound(
                    category_id,
                ))?,
            &source_locale,
            &target_locale,
        )?;
        ensure_revision(
            "resource",
            &request.expected_resource_revision,
            &before.resource_revision,
        )?;
        ensure_revision(
            "source",
            &request.expected_source_revision,
            &before.source_revision,
        )?;
        if request.expected_target_revision != before.target_revision {
            return Err(ProductCategorySeoTranslationError::RevisionConflict {
                revision: "target",
            });
        }

        txn.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
INSERT INTO catalog_category_seo_translations (
    tenant_id, category_id, locale, meta_title, meta_description, created_at, updated_at
) VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
ON CONFLICT (tenant_id, category_id, locale) DO UPDATE SET
    meta_title = EXCLUDED.meta_title,
    meta_description = EXCLUDED.meta_description,
    updated_at = CURRENT_TIMESTAMP
"#,
            vec![
                tenant_id.into(),
                category_id.into(),
                target_locale.clone().into(),
                request.meta_title.clone().into(),
                request.meta_description.clone().into(),
            ],
        ))
        .await?;

        let after = build_snapshot(
            load_category_seo_aggregate(&txn, tenant_id, category_id)
                .await?
                .filter(|aggregate| !aggregate.deleted && aggregate.has_copy())
                .ok_or(ProductCategorySeoTranslationError::CategorySeoNotFound(
                    category_id,
                ))?,
            &source_locale,
            &target_locale,
        )?;
        let target = after.target.clone().ok_or_else(|| {
            ProductCategorySeoTranslationError::OwnerInvariant(
                "exact target locale is absent after Product Category SEO translation apply"
                    .to_string(),
            )
        })?;
        validate_copy(&target)?;
        let target_revision = after.target_revision.clone().ok_or_else(|| {
            ProductCategorySeoTranslationError::OwnerInvariant(
                "exact target revision is absent after Product Category SEO translation apply"
                    .to_string(),
            )
        })?;

        txn.publish(
            tenant_id,
            actor_user_id,
            DomainEvent::CatalogCategoryUpdated { category_id },
        )
        .await?;
        let receipt = ExactLocaleApplyReceipt {
            operation_id: current_product_operation_id(),
            category_id,
            resource_revision: after.resource_revision,
            target_revision,
            target,
        };
        record_product_operation_result(&receipt)?;
        txn.commit().await?;
        Ok(receipt)
    }

    async fn read_category_seo_translation_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> OwnerResult<ProgressFacts> {
        validate_uuid(tenant_id, "tenant_id")?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        ensure_postgres(self.db.get_database_backend())?;
        let txn = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await?;
        let row = txn
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
SELECT
    COUNT(*)::BIGINT AS resources,
    COALESCE(SUM(
        (CASE WHEN NULLIF(BTRIM(source.meta_title), '') IS NOT NULL THEN 1 ELSE 0 END) +
        (CASE WHEN NULLIF(BTRIM(source.meta_description), '') IS NOT NULL THEN 1 ELSE 0 END)
    ), 0)::BIGINT AS optional_units,
    COALESCE(SUM(
        (CASE WHEN NULLIF(BTRIM(source.meta_title), '') IS NOT NULL
                   AND NULLIF(BTRIM(target.meta_title), '') IS NOT NULL THEN 1 ELSE 0 END) +
        (CASE WHEN NULLIF(BTRIM(source.meta_description), '') IS NOT NULL
                   AND NULLIF(BTRIM(target.meta_description), '') IS NOT NULL THEN 1 ELSE 0 END)
    ), 0)::BIGINT AS exact_optional_units,
    COUNT(*) FILTER (
        WHERE (NULLIF(BTRIM(source.meta_title), '') IS NULL
               OR NULLIF(BTRIM(target.meta_title), '') IS NOT NULL)
          AND (NULLIF(BTRIM(source.meta_description), '') IS NULL
               OR NULLIF(BTRIM(target.meta_description), '') IS NOT NULL)
    )::BIGINT AS complete_resources
FROM catalog_categories category
INNER JOIN catalog_category_seo_translations source
    ON source.tenant_id = category.tenant_id
   AND source.category_id = category.id
   AND source.locale = $2
LEFT JOIN catalog_category_seo_translations target
    ON target.tenant_id = category.tenant_id
   AND target.category_id = category.id
   AND target.locale = $3
WHERE category.tenant_id = $1
  AND category.deleted_at IS NULL
  AND (
      NULLIF(BTRIM(source.meta_title), '') IS NOT NULL
      OR NULLIF(BTRIM(source.meta_description), '') IS NOT NULL
  )
"#,
                vec![
                    tenant_id.into(),
                    source_locale.into(),
                    target_locale.into(),
                ],
            ))
            .await?
            .ok_or_else(|| {
                ProductCategorySeoTranslationError::OwnerInvariant(
                    "progress aggregate query returned no row".to_string(),
                )
            })?;
        let facts = ProgressFacts {
            resources: nonnegative_count(row.try_get("", "resources")?, "resources")?,
            optional_units: nonnegative_count(
                row.try_get("", "optional_units")?,
                "optional_units",
            )?,
            exact_optional_units: nonnegative_count(
                row.try_get("", "exact_optional_units")?,
                "exact_optional_units",
            )?,
            complete_resources: nonnegative_count(
                row.try_get("", "complete_resources")?,
                "complete_resources",
            )?,
        };
        txn.commit().await?;
        Ok(facts)
    }

    async fn category_seo_translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> OwnerResult<Option<u64>> {
        validate_uuid(tenant_id, "tenant_id")?;
        ensure_postgres(self.db.get_database_backend())?;
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT MAX(change_seq) AS highwater FROM product_category_seo_translation_change_journal WHERE tenant_id = $1",
                vec![tenant_id.into()],
            ))
            .await?
            .ok_or_else(|| {
                ProductCategorySeoTranslationError::OwnerInvariant(
                    "change high-water query returned no row".to_string(),
                )
            })?;
        optional_positive_sequence(row.try_get("", "highwater")?, "high-water")
    }

    async fn read_category_seo_translation_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> OwnerResult<Vec<ChangeRecord>> {
        validate_uuid(tenant_id, "tenant_id")?;
        if through_seq == 0 || after_seq > through_seq {
            return Err(CommerceError::Validation(
                "Product Category SEO translation change cursor bounds are invalid".to_string(),
            )
            .into());
        }
        if limit == 0 || limit > MAX_CHANGE_PAGE {
            return Err(CommerceError::Validation(format!(
                "Product Category SEO translation change page size must be between 1 and {MAX_CHANGE_PAGE}"
            ))
            .into());
        }
        ensure_postgres(self.db.get_database_backend())?;
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
SELECT change_seq, category_id, resource_revision, lifecycle
FROM product_category_seo_translation_change_journal
WHERE tenant_id = $1
  AND change_seq > $2
  AND change_seq <= $3
ORDER BY change_seq ASC
LIMIT $4
"#,
                vec![
                    tenant_id.into(),
                    i64::try_from(after_seq)
                        .map_err(|_| invalid_sequence("after"))?
                        .into(),
                    i64::try_from(through_seq)
                        .map_err(|_| invalid_sequence("through"))?
                        .into(),
                    i64::from(limit).into(),
                ],
            ))
            .await?;
        rows.into_iter().map(change_record_from_row).collect()
    }

    pub(crate) async fn record_category_seo_translation_change_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        root_event_id: Uuid,
    ) -> CommerceResult<()> {
        if txn.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }
        if tenant_id.is_nil() || category_id.is_nil() || root_event_id.is_nil() {
            return Err(CommerceError::Validation(
                "Product Category SEO translation change identity must not be nil".to_string(),
            ));
        }

        let aggregate = load_category_seo_aggregate(txn, tenant_id, category_id)
            .await
            .map_err(owner_error_to_commerce)?;
        let previous = PreviousChangeRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
SELECT resource_revision, lifecycle
FROM product_category_seo_translation_change_journal
WHERE tenant_id = $1 AND category_id = $2
ORDER BY change_seq DESC
LIMIT 1
"#,
            vec![tenant_id.into(), category_id.into()],
        ))
        .one(txn)
        .await?;

        let current = match aggregate {
            Some(aggregate) if aggregate.has_copy() && !aggregate.deleted => Some((
                resource_revision(aggregate.translation_revision),
                if aggregate.is_active {
                    ChangeLifecycle::Active
                } else {
                    ChangeLifecycle::Archived
                },
            )),
            Some(aggregate) if aggregate.has_copy() && aggregate.deleted => Some((
                deleted_revision(root_event_id, category_id),
                ChangeLifecycle::Deleted,
            )),
            _ => None,
        };

        match (current, previous) {
            (Some((revision, lifecycle)), Some(previous))
                if previous.resource_revision == revision
                    && previous.lifecycle == lifecycle.as_str() =>
            {
                Ok(())
            }
            (Some((revision, lifecycle)), _) => {
                insert_change(
                    txn,
                    root_event_id,
                    tenant_id,
                    category_id,
                    &revision,
                    lifecycle,
                )
                .await
            }
            (None, None) => Ok(()),
            (None, Some(previous)) if previous.lifecycle == ChangeLifecycle::Deleted.as_str() => {
                Ok(())
            }
            (None, Some(_)) => {
                let revision = deleted_revision(root_event_id, category_id);
                insert_change(
                    txn,
                    root_event_id,
                    tenant_id,
                    category_id,
                    &revision,
                    ChangeLifecycle::Deleted,
                )
                .await
            }
        }
    }
}

#[async_trait]
impl TranslationTargetProvider for ProductCategorySeoTranslationTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Product owner slug must satisfy Translation contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Product Category SEO resource kind must satisfy Translation contract"),
            display_name: "Product Category SEO copy".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::AggregateProgress,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
                TranslationTargetCapability::ChangeCursor,
            ]),
            read_permission_floor: BTreeSet::from(["products:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["products:update".to_string()]),
        }
    }

    async fn list_resources(
        &self,
        context: PortContext,
        request: ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let after = request
            .cursor
            .as_ref()
            .map(|cursor| parse_resource_cursor(cursor.as_str()))
            .transpose()?;
        let (resources, next_after) = self
            .service
            .list_category_seo_translation_exact_resources(
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
                after,
                request.limit,
            )
            .await
            .map_err(owner_error_to_port_error)?;
        let resources = resources
            .iter()
            .map(summary_from_owner)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(TranslationResourcePage {
            resources,
            next_cursor: next_after.map(resource_cursor).transpose()?,
        })
    }

    async fn read_resource(
        &self,
        context: PortContext,
        request: ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        let tenant_id = parse_tenant_id(&context)?;
        let owner = load_owner_snapshot(&self.service, tenant_id, &request).await?;
        snapshot_from_owner(&owner, &request)
    }

    async fn read_progress(
        &self,
        context: PortContext,
        request: TranslationTargetProgressRequest,
    ) -> Result<TranslationTargetProgressFacts, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        for _ in 0..PROGRESS_STABILITY_ATTEMPTS {
            let before = self
                .service
                .category_seo_translation_change_highwater(tenant_id)
                .await
                .map_err(owner_error_to_port_error)?;
            let owner = self
                .service
                .read_category_seo_translation_progress(
                    tenant_id,
                    request.source_locale.as_str(),
                    request.target_locale.as_str(),
                )
                .await
                .map_err(owner_error_to_port_error)?;
            let after = self
                .service
                .category_seo_translation_change_highwater(tenant_id)
                .await
                .map_err(owner_error_to_port_error)?;
            if before != after {
                continue;
            }
            let facts = TranslationTargetProgressFacts {
                required_units: 0,
                exact_required_units: 0,
                optional_units: owner.optional_units,
                exact_optional_units: owner.exact_optional_units,
                resources: owner.resources,
                complete_resources: owner.complete_resources,
                owner_change_cursor: after
                    .map(|change_seq| change_cursor(change_seq, change_seq))
                    .transpose()?,
            };
            facts.validate().map_err(|error| {
                PortError::invariant_violation(
                    "product.category_seo_translation_progress_invalid",
                    error.to_string(),
                )
            })?;
            return Ok(facts);
        }
        Err(PortError::unavailable(
            "product.category_seo_translation_progress_unstable",
            "Product Category SEO translation progress changed while it was being aggregated",
        ))
    }

    async fn read_changes(
        &self,
        context: PortContext,
        request: TranslationTargetChangesRequest,
    ) -> Result<TranslationTargetChangePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let parsed = request.after.as_ref().map(parse_change_cursor).transpose()?;
        let (through, after) = match parsed {
            Some((through, after)) if through == after => {
                let current = self
                    .service
                    .category_seo_translation_change_highwater(tenant_id)
                    .await
                    .map_err(owner_error_to_port_error)?
                    .unwrap_or(after)
                    .max(after);
                (current, after)
            }
            Some(cursor) => cursor,
            None => (
                self.service
                    .category_seo_translation_change_highwater(tenant_id)
                    .await
                    .map_err(owner_error_to_port_error)?
                    .unwrap_or(0),
                0,
            ),
        };
        if through == 0 {
            return Ok(TranslationTargetChangePage {
                changes: Vec::new(),
                next_cursor: None,
            });
        }
        let owner_changes = self
            .service
            .read_category_seo_translation_changes(tenant_id, after, through, request.limit)
            .await
            .map_err(owner_error_to_port_error)?;
        let last_seq = owner_changes.last().map(|change| change.change_seq);
        let next_cursor = Some(match last_seq {
            Some(last_seq) if last_seq < through => change_cursor(through, last_seq)?,
            _ => change_cursor(through, through)?,
        });
        let changes = owner_changes
            .into_iter()
            .map(|change| {
                Ok(TranslationTargetChange {
                    identity: category_seo_identity(change.category_id),
                    resource_revision: opaque_revision(change.resource_revision, "resource_revision")?,
                    lifecycle: match change.lifecycle {
                        ChangeLifecycle::Active => TranslationResourceLifecycle::Active,
                        ChangeLifecycle::Archived => TranslationResourceLifecycle::Archived,
                        ChangeLifecycle::Deleted => TranslationResourceLifecycle::Deleted,
                    },
                })
            })
            .collect::<Result<Vec<_>, PortError>>()?;
        Ok(TranslationTargetChangePage {
            changes,
            next_cursor,
        })
    }

    async fn validate_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Update)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let read_request = read_request_from_patch(&request);
        let owner = load_owner_snapshot(&self.service, tenant_id, &read_request).await?;
        let snapshot = snapshot_from_owner(&owner, &read_request)?;
        let mut validation = validate_patch_against_snapshot(&request, &snapshot);
        if validation.accepted {
            if let Err(error) = merged_target(&request, &snapshot) {
                validation.accepted = false;
                validation.issues.push(rustok_translation_targets::TranslationPatchIssue {
                    code: "product.category_seo_translation_target_empty".to_string(),
                    message: error.message,
                    severity: rustok_translation_targets::TranslationPatchIssueSeverity::Error,
                    field_key: None,
                });
            }
        }
        Ok(validation)
    }

    async fn apply_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        validate_translation_apply_context(&context)?;
        let security = authorize(&context, Action::Update)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = parse_tenant_id(&context)?;
        let category_id = parse_identity(&request.identity)?;
        let idempotency_key = context.idempotency_key.as_deref().unwrap_or_default();
        let lease = match self
            .service
            .admit_schema_operation_receipt(
                tenant_id,
                idempotency_key,
                OPERATION_APPLY_PATCH,
                &request,
            )
            .await?
        {
            Admission::Run(lease) => lease,
            Admission::Replay(value) => {
                let receipt = decode_owner_receipt(value)?;
                return application_receipt(&receipt, &request);
            }
            Admission::ReplayError(error) => return Err(error),
        };

        let result = async {
            let read_request = read_request_from_patch(&request);
            let owner = load_owner_snapshot(&self.service, tenant_id, &read_request).await?;
            let snapshot = snapshot_from_owner(&owner, &read_request)?;
            let validation = validate_patch_against_snapshot(&request, &snapshot);
            if !validation.accepted {
                return Err(validation_to_port_error(&validation));
            }
            let target = merged_target(&request, &snapshot)?;
            let applied = with_product_operation_receipt(
                lease,
                self.service.apply_category_seo_translation_exact_locale(
                    tenant_id,
                    security.user_id,
                    category_id,
                    target,
                ),
            )
            .await
            .map_err(owner_error_to_port_error)?;
            if applied.operation_id != Some(lease.operation_id) {
                return Err(PortError::invariant_violation(
                    "product.category_seo_translation_receipt_identity_invalid",
                    "Product Category SEO owner receipt is not bound to the active Translation operation",
                ));
            }
            application_receipt(&applied, &request)
        }
        .await;
        if let Err(error) = &result {
            self.service.fail_schema_operation_receipt(lease, error).await?;
        }
        result
    }
}

async fn load_owner_snapshot(
    service: &ProductCatalogSchemaService,
    tenant_id: Uuid,
    request: &ReadTranslationResourceRequest,
) -> Result<ExactLocaleSnapshot, PortError> {
    let category_id = parse_identity(&request.identity)?;
    service
        .read_category_seo_translation_exact_locale(
            tenant_id,
            category_id,
            request.source_locale.as_str(),
            request.target_locale.as_str(),
        )
        .await
        .map_err(owner_error_to_port_error)
}

async fn load_source_eligible_page<C>(
    db: &C,
    tenant_id: Uuid,
    source_locale: &str,
    after: Option<Uuid>,
    limit: u16,
) -> OwnerResult<Vec<Aggregate>>
where
    C: ConnectionTrait,
{
    let statement = match after {
        Some(after) => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            source_eligible_sql(true),
            vec![
                tenant_id.into(),
                source_locale.to_string().into(),
                after.into(),
                i64::from(limit).into(),
            ],
        ),
        None => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            source_eligible_sql(false),
            vec![
                tenant_id.into(),
                source_locale.to_string().into(),
                i64::from(limit).into(),
            ],
        ),
    };
    AggregateRow::find_by_statement(statement)
        .all(db)
        .await?
        .into_iter()
        .map(decode_aggregate)
        .collect()
}

async fn load_category_seo_aggregate<C>(
    db: &C,
    tenant_id: Uuid,
    category_id: Uuid,
) -> OwnerResult<Option<Aggregate>>
where
    C: ConnectionTrait,
{
    AggregateRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!("{} WHERE category.tenant_id = $1 AND category.id = $2", aggregate_select_sql()),
        vec![tenant_id.into(), category_id.into()],
    ))
    .one(db)
    .await?
    .map(decode_aggregate)
    .transpose()
}

fn aggregate_select_sql() -> &'static str {
    r#"
SELECT
    category.id,
    category.tenant_id,
    category.code,
    category.is_active,
    category.deleted_at IS NOT NULL AS deleted,
    category.category_seo_translation_revision AS translation_revision,
    COALESCE((
        SELECT jsonb_agg(
            jsonb_build_object(
                'locale', seo.locale,
                'meta_title', seo.meta_title,
                'meta_description', seo.meta_description
            ) ORDER BY seo.locale
        )
        FROM catalog_category_seo_translations seo
        WHERE seo.tenant_id = category.tenant_id
          AND seo.category_id = category.id
    ), '[]'::jsonb) AS translations
FROM catalog_categories category
"#
}

fn source_eligible_sql(with_after: bool) -> String {
    let after = if with_after { "AND category.id > $3" } else { "" };
    let limit = if with_after { "$4" } else { "$3" };
    format!(
        r#"
WITH candidate AS (
    SELECT category.id
    FROM catalog_categories category
    INNER JOIN catalog_category_seo_translations source
        ON source.tenant_id = category.tenant_id
       AND source.category_id = category.id
       AND source.locale = $2
    WHERE category.tenant_id = $1
      AND category.deleted_at IS NULL
      AND (
          NULLIF(BTRIM(source.meta_title), '') IS NOT NULL
          OR NULLIF(BTRIM(source.meta_description), '') IS NOT NULL
      )
      {after}
    ORDER BY category.id ASC
    LIMIT {limit}
)
{}
INNER JOIN candidate ON candidate.id = category.id
ORDER BY category.id ASC
"#,
        aggregate_select_sql()
    )
}

fn decode_aggregate(row: AggregateRow) -> OwnerResult<Aggregate> {
    let translations: Vec<StoredTranslation> = serde_json::from_value(row.translations).map_err(|error| {
        ProductCategorySeoTranslationError::OwnerInvariant(format!(
            "persisted Product Category SEO translations are invalid: {error}"
        ))
    })?;
    if row.id.is_nil() || row.tenant_id.is_nil() || row.code.trim().is_empty() {
        return Err(ProductCategorySeoTranslationError::OwnerInvariant(
            "persisted Product Category SEO identity is invalid".to_string(),
        ));
    }
    let translation_revision = positive_sequence(row.translation_revision, "resource")?;
    for translation in &translations {
        validate_stored_translation(translation)?;
    }
    Ok(Aggregate {
        id: row.id,
        tenant_id: row.tenant_id,
        code: row.code,
        is_active: row.is_active,
        deleted: row.deleted,
        translation_revision,
        translations,
    })
}

fn build_snapshot(
    aggregate: Aggregate,
    source_locale: &str,
    target_locale: &str,
) -> OwnerResult<ExactLocaleSnapshot> {
    let source_record = aggregate
        .translations
        .iter()
        .find(|translation| translation.locale == source_locale)
        .filter(|translation| translation.has_copy())
        .ok_or_else(|| ProductCategorySeoTranslationError::SourceLocaleNotFound {
            category_id: aggregate.id,
            locale: source_locale.to_string(),
        })?;
    let source = source_record.copy();
    let target_record = aggregate
        .translations
        .iter()
        .find(|translation| translation.locale == target_locale)
        .filter(|translation| translation.has_copy());
    let target = target_record.map(StoredTranslation::copy);
    let target_revision = target_record.map(|translation| {
        locale_revision(
            aggregate.tenant_id,
            aggregate.id,
            target_locale,
            &translation.copy(),
        )
    });
    let exact_locales = aggregate
        .translations
        .iter()
        .filter(|translation| translation.has_copy())
        .map(|translation| translation.locale.clone())
        .collect::<Vec<_>>();
    Ok(ExactLocaleSnapshot {
        category_id: aggregate.id,
        category_code: aggregate.code,
        is_active: aggregate.is_active,
        source_locale: source_locale.to_string(),
        target_locale: target_locale.to_string(),
        resource_revision: resource_revision(aggregate.translation_revision),
        source_revision: locale_revision(
            aggregate.tenant_id,
            aggregate.id,
            source_locale,
            &source,
        ),
        target_revision,
        exact_locales,
        source,
        target,
    })
}

fn resource_revision(sequence: u64) -> String {
    format!("{RESOURCE_REVISION_PREFIX}{sequence}")
}

fn locale_revision(tenant_id: Uuid, category_id: Uuid, locale: &str, copy: &SeoCopy) -> String {
    let mut hasher = Sha256::new();
    hash_str(&mut hasher, LOCALE_REVISION_NAMESPACE);
    hash_uuid(&mut hasher, tenant_id);
    hash_uuid(&mut hasher, category_id);
    hash_str(&mut hasher, locale);
    hash_optional_str(&mut hasher, copy.meta_title.as_deref());
    hash_optional_str(&mut hasher, copy.meta_description.as_deref());
    format!("product-category-seo-locale-v1:{}", hex::encode(hasher.finalize()))
}

fn deleted_revision(root_event_id: Uuid, category_id: Uuid) -> String {
    let mut hasher = Sha256::new();
    hash_str(&mut hasher, DELETED_REVISION_NAMESPACE);
    hash_uuid(&mut hasher, root_event_id);
    hash_uuid(&mut hasher, category_id);
    format!("product-category-seo-deleted-v1:{}", hex::encode(hasher.finalize()))
}

fn summary_from_owner(owner: &ExactLocaleSnapshot) -> Result<TranslationResourceSummary, PortError> {
    Ok(TranslationResourceSummary {
        identity: category_seo_identity(owner.category_id),
        display_label: owner.category_code.clone(),
        lifecycle: if owner.is_active {
            TranslationResourceLifecycle::Active
        } else {
            TranslationResourceLifecycle::Archived
        },
        resource_revision: opaque_revision(owner.resource_revision.clone(), "resource_revision")?,
        exact_locales: owner
            .exact_locales
            .iter()
            .map(|locale| {
                TenantLocale::new(locale.clone()).map_err(|error| {
                    PortError::invariant_violation(
                        "product.category_seo_translation_locale_invalid",
                        error.to_string(),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn snapshot_from_owner(
    owner: &ExactLocaleSnapshot,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError> {
    if owner.source_locale != request.source_locale.as_str()
        || owner.target_locale != request.target_locale.as_str()
    {
        return Err(PortError::invariant_violation(
            "product.category_seo_translation_locale_identity_invalid",
            "Product Category SEO owner returned different exact locales",
        ));
    }
    let target = owner.target.as_ref();
    let fields = vec![
        seo_field(
            META_TITLE_FIELD,
            owner.source.meta_title.as_deref().unwrap_or_default(),
            target.and_then(|copy| copy.meta_title.clone()),
            Some(255),
        ),
        seo_field(
            META_DESCRIPTION_FIELD,
            owner.source.meta_description.as_deref().unwrap_or_default(),
            target.and_then(|copy| copy.meta_description.clone()),
            Some(500),
        ),
    ];
    let snapshot = TranslationResourceSnapshot {
        summary: summary_from_owner(owner)?,
        source_locale: request.source_locale.clone(),
        target_locale: request.target_locale.clone(),
        rendered_fallback_locale: None,
        source_revision: opaque_revision(owner.source_revision.clone(), "source_revision")?,
        target_revision: owner
            .target_revision
            .clone()
            .map(|revision| opaque_revision(revision, "target_revision"))
            .transpose()?,
        fields,
    };
    snapshot.validate().map_err(|error| {
        PortError::invariant_violation(
            "product.category_seo_translation_snapshot_invalid",
            error.to_string(),
        )
    })?;
    Ok(snapshot)
}

fn seo_field(
    key: &'static str,
    source_value: &str,
    exact_target_value: Option<String>,
    max_characters: Option<u32>,
) -> TranslationFieldSnapshot {
    TranslationFieldSnapshot {
        descriptor: TranslationFieldDescriptor {
            key: FieldKey::new(key).expect("static Product Category SEO field must be valid"),
            profile: TranslationValueProfile::SeoText,
            strategy: TranslationStrategy::Translate,
            classification: TranslationDataClassification::Public,
            required: false,
            ai_export_allowed: true,
            max_characters,
            preserves_whitespace: false,
        },
        source_value: source_value.to_string(),
        exact_target_value,
        source_hash: field_hash(source_value),
        protected_tokens: Vec::new(),
    }
}

fn merged_target(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Result<ExactLocaleApply, PortError> {
    let mut values = merged_patch_values(request, snapshot);
    let meta_title = values
        .remove(META_TITLE_FIELD)
        .flatten()
        .and_then(normalize_optional_target_value);
    let meta_description = values
        .remove(META_DESCRIPTION_FIELD)
        .flatten()
        .and_then(normalize_optional_target_value);
    if !values.is_empty() {
        return Err(PortError::validation(
            "product.category_seo_translation_patch_field_unknown",
            "Product Category SEO translation patch contains an unknown field",
        ));
    }
    let copy = SeoCopy {
        meta_title: meta_title.clone(),
        meta_description: meta_description.clone(),
    };
    validate_copy_port(&copy)?;
    Ok(ExactLocaleApply {
        source_locale: request.source_locale.as_str().to_string(),
        target_locale: request.target_locale.as_str().to_string(),
        meta_title,
        meta_description,
        expected_resource_revision: request.expected_resource_revision.as_str().to_string(),
        expected_source_revision: request.expected_source_revision.as_str().to_string(),
        expected_target_revision: request
            .expected_target_revision
            .as_ref()
            .map(|revision| revision.as_str().to_string()),
    })
}

fn decode_owner_receipt(value: serde_json::Value) -> Result<ExactLocaleApplyReceipt, PortError> {
    serde_json::from_value(value).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_corrupt", error.to_string())
    })
}

fn application_receipt(
    owner_receipt: &ExactLocaleApplyReceipt,
    request: &TranslationPatchRequest,
) -> Result<TranslationApplicationReceipt, PortError> {
    let operation_id = owner_receipt.operation_id.ok_or_else(|| {
        PortError::invariant_violation(
            "product.category_seo_translation_receipt_identity_missing",
            "Product Category SEO owner receipt is missing operation identity",
        )
    })?;
    Ok(TranslationApplicationReceipt {
        provider_receipt_id: format!("product-category-seo:{operation_id}"),
        resource_revision: opaque_revision(
            owner_receipt.resource_revision.clone(),
            "resource_revision",
        )?,
        target_revision: opaque_revision(owner_receipt.target_revision.clone(), "target_revision")?,
        applied_field_keys: request
            .fields
            .iter()
            .map(|field: &TranslationFieldPatch| field.key.clone())
            .collect(),
    })
}

async fn insert_change(
    txn: &DatabaseTransaction,
    root_event_id: Uuid,
    tenant_id: Uuid,
    category_id: Uuid,
    resource_revision: &str,
    lifecycle: ChangeLifecycle,
) -> CommerceResult<()> {
    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
INSERT INTO product_category_seo_translation_change_journal (
    root_event_id, tenant_id, category_id, resource_revision, lifecycle, created_at
) VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP)
ON CONFLICT (root_event_id, category_id) DO NOTHING
"#,
        vec![
            root_event_id.into(),
            tenant_id.into(),
            category_id.into(),
            resource_revision.to_string().into(),
            lifecycle.as_str().into(),
        ],
    ))
    .await?;
    Ok(())
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "product.invalid_tenant_id",
            "Product Category SEO translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Products, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "product.category_seo_translation_permission_denied",
            format!("products:{action} permission is required"),
        ));
    }
    Ok(security)
}

fn parse_identity(identity: &TranslationResourceIdentity) -> Result<Uuid, PortError> {
    if identity.owner_slug.as_str() != TRANSLATION_OWNER_SLUG
        || identity.resource_kind.as_str() != TRANSLATION_RESOURCE_KIND
        || identity.subresource_id.is_some()
    {
        return Err(PortError::validation(
            "product.category_seo_translation_identity_invalid",
            "Product Category SEO identity must address product/category_seo without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "product.category_seo_translation_resource_id_invalid",
            "Product Category SEO resource id must be a UUID",
        )
    })
}

fn category_seo_identity(category_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Product owner slug must satisfy Translation contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Product Category SEO resource kind must satisfy Translation contract"),
        resource_id: ResourceId::new(category_id.to_string())
            .expect("Product Category UUID must satisfy Translation resource id contract"),
        subresource_id: None,
    }
}

fn resource_cursor(category_id: Uuid) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(category_id.to_string()).map_err(|error| {
        PortError::invariant_violation(
            "product.category_seo_translation_resource_cursor_invalid",
            error.to_string(),
        )
    })
}

fn parse_resource_cursor(value: &str) -> Result<Uuid, PortError> {
    Uuid::parse_str(value).map_err(|_| {
        PortError::validation(
            "product.category_seo_translation_resource_cursor_invalid",
            "Product Category SEO resource cursor must be a UUID",
        )
    })
}

fn change_cursor(through: u64, after: u64) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(format!("{CHANGE_CURSOR_VERSION}:{through}:{after}")).map_err(|error| {
        PortError::invariant_violation(
            "product.category_seo_translation_change_cursor_invalid",
            error.to_string(),
        )
    })
}

fn parse_change_cursor(cursor: &OpaqueCursor) -> Result<(u64, u64), PortError> {
    let mut parts = cursor.as_str().split(':');
    let version = parts.next();
    let through = parts.next().and_then(|value| value.parse::<u64>().ok());
    let after = parts.next().and_then(|value| value.parse::<u64>().ok());
    if version != Some(CHANGE_CURSOR_VERSION)
        || parts.next().is_some()
        || through.is_none()
        || after.is_none()
    {
        return Err(PortError::validation(
            "product.category_seo_translation_change_cursor_invalid",
            "Product Category SEO translation change cursor is invalid",
        ));
    }
    let through = through.unwrap_or_default();
    let after = after.unwrap_or_default();
    if through == 0 || after == 0 || after > through {
        return Err(PortError::validation(
            "product.category_seo_translation_change_cursor_invalid",
            "Product Category SEO translation change cursor bounds are invalid",
        ));
    }
    Ok((through, after))
}

fn opaque_revision(value: String, field: &'static str) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(value).map_err(|error| {
        PortError::invariant_violation(
            "product.category_seo_translation_revision_invalid",
            format!("Product Category SEO {field} is invalid: {error}"),
        )
    })
}

fn owner_error_to_port_error(error: ProductCategorySeoTranslationError) -> PortError {
    match error {
        ProductCategorySeoTranslationError::CategorySeoNotFound(_) => PortError::not_found(
            "product.category_seo_translation_resource_not_found",
            "Product Category SEO translation resource was not found",
        ),
        ProductCategorySeoTranslationError::SourceLocaleNotFound { .. } => PortError::not_found(
            "product.category_seo_translation_source_not_found",
            "Exact source Product Category SEO locale was not found",
        ),
        ProductCategorySeoTranslationError::RevisionConflict { .. } => PortError::conflict(
            "product.category_seo_translation_revision_conflict",
            "Product Category SEO state conflicts with the requested mutation",
        ),
        ProductCategorySeoTranslationError::OwnerInvariant(_) => PortError::invariant_violation(
            "product.category_seo_translation_owner_invariant",
            "Product Category SEO owner state is invalid",
        ),
        ProductCategorySeoTranslationError::Commerce(error) => product_error_to_port_error(error),
    }
}

fn product_error_to_port_error(error: CommerceError) -> PortError {
    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "product.category_seo_translation_owner_unavailable",
            "Product Category SEO storage is temporarily unavailable",
        ),
        CommerceError::Validation(_) => PortError::validation(
            "product.category_seo_translation_owner_validation",
            "Product rejected the Category SEO translation mutation",
        ),
        CommerceError::ProductNotFound(_) | CommerceError::VariantNotFound(_) => PortError::not_found(
            "product.category_seo_translation_resource_not_found",
            "Product Category SEO translation resource was not found",
        ),
        CommerceError::DuplicateHandle { .. }
        | CommerceError::DuplicateSku(_)
        | CommerceError::CannotDeleteOnlyVariant
        | CommerceError::CannotDeletePublished => PortError::conflict(
            "product.category_seo_translation_owner_conflict",
            "Product state conflicts with the requested Category SEO translation mutation",
        ),
        CommerceError::NoVariants => PortError::validation(
            "product.category_seo_translation_owner_validation",
            "Product rejected the Category SEO translation mutation",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "product.category_seo_translation_owner_invariant",
            "Product Category SEO state is invalid",
        ),
    }
}

fn canonical_locale(locale: &str) -> OwnerResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| CommerceError::Validation(error.to_string()).into())
}

fn validate_locale_pair(source_locale: &str, target_locale: &str) -> OwnerResult<()> {
    if source_locale == target_locale {
        return Err(CommerceError::Validation(
            "Product Category SEO translation source and target locale must differ".to_string(),
        )
        .into());
    }
    Ok(())
}

fn validate_stored_translation(translation: &StoredTranslation) -> OwnerResult<()> {
    if canonical_locale(&translation.locale)? != translation.locale {
        return Err(ProductCategorySeoTranslationError::OwnerInvariant(
            "persisted Product Category SEO translation locale is not canonical".to_string(),
        ));
    }
    if translation
        .meta_title
        .as_deref()
        .is_some_and(|value| value.chars().count() > 255)
        || translation
            .meta_description
            .as_deref()
            .is_some_and(|value| value.chars().count() > 500)
    {
        return Err(ProductCategorySeoTranslationError::OwnerInvariant(
            "persisted Product Category SEO copy exceeds its storage bounds".to_string(),
        ));
    }
    Ok(())
}

fn validate_copy(copy: &SeoCopy) -> OwnerResult<()> {
    if !copy.has_copy() {
        return Err(CommerceError::Validation(
            "Product Category SEO target locale must contain meta_title or meta_description"
                .to_string(),
        )
        .into());
    }
    if copy
        .meta_title
        .as_deref()
        .is_some_and(|value| value.chars().count() > 255)
    {
        return Err(CommerceError::Validation(
            "Product Category SEO meta_title must not exceed 255 characters".to_string(),
        )
        .into());
    }
    if copy
        .meta_description
        .as_deref()
        .is_some_and(|value| value.chars().count() > 500)
    {
        return Err(CommerceError::Validation(
            "Product Category SEO meta_description must not exceed 500 characters".to_string(),
        )
        .into());
    }
    Ok(())
}

fn validate_copy_port(copy: &SeoCopy) -> Result<(), PortError> {
    validate_copy(copy).map_err(owner_error_to_port_error)
}

fn validate_uuid(value: Uuid, field: &str) -> OwnerResult<()> {
    if value.is_nil() {
        return Err(CommerceError::Validation(format!(
            "Product Category SEO translation {field} must not be nil"
        ))
        .into());
    }
    Ok(())
}

fn ensure_revision(revision: &'static str, expected: &str, current: &str) -> OwnerResult<()> {
    if expected != current {
        return Err(ProductCategorySeoTranslationError::RevisionConflict { revision });
    }
    Ok(())
}

fn ensure_postgres(backend: DatabaseBackend) -> OwnerResult<()> {
    if backend != DatabaseBackend::Postgres {
        return Err(CommerceError::Validation(
            "Product Category SEO Translation target requires PostgreSQL".to_string(),
        )
        .into());
    }
    Ok(())
}

fn nonblank(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

fn nonnegative_count(value: i64, field: &str) -> OwnerResult<u64> {
    u64::try_from(value).map_err(|_| {
        ProductCategorySeoTranslationError::OwnerInvariant(format!(
            "Product Category SEO translation {field} count must be nonnegative"
        ))
    })
}

fn optional_positive_sequence(value: Option<i64>, field: &str) -> OwnerResult<Option<u64>> {
    value.map(|value| positive_sequence(value, field)).transpose()
}

fn positive_sequence(value: i64, field: &str) -> OwnerResult<u64> {
    let value = u64::try_from(value).map_err(|_| invalid_sequence(field))?;
    if value == 0 {
        return Err(invalid_sequence(field));
    }
    Ok(value)
}

fn invalid_sequence(field: &str) -> ProductCategorySeoTranslationError {
    ProductCategorySeoTranslationError::OwnerInvariant(format!(
        "Product Category SEO translation change {field} sequence must be positive"
    ))
}

fn change_record_from_row(row: sea_orm::QueryResult) -> OwnerResult<ChangeRecord> {
    let change_seq = positive_sequence(row.try_get("", "change_seq")?, "change")?;
    let category_id: Uuid = row.try_get("", "category_id")?;
    let resource_revision: String = row.try_get("", "resource_revision")?;
    let lifecycle: String = row.try_get("", "lifecycle")?;
    if category_id.is_nil() || resource_revision.trim().is_empty() {
        return Err(ProductCategorySeoTranslationError::OwnerInvariant(
            "Product Category SEO translation change row is invalid".to_string(),
        ));
    }
    Ok(ChangeRecord {
        change_seq,
        category_id,
        resource_revision,
        lifecycle: ChangeLifecycle::parse(&lifecycle)?,
    })
}

fn owner_error_to_commerce(error: ProductCategorySeoTranslationError) -> CommerceError {
    match error {
        ProductCategorySeoTranslationError::Commerce(error) => error,
        error => CommerceError::Validation(error.to_string()),
    }
}

fn hash_str(hasher: &mut Sha256, value: &str) {
    hash_len(hasher, value.len());
    hasher.update(value.as_bytes());
}

fn hash_optional_str(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hash_str(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn hash_uuid(hasher: &mut Sha256, value: Uuid) {
    hasher.update(value.as_bytes());
}

fn hash_len(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_be_bytes());
}
