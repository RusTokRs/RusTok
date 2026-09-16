use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource, TenantLocale};
use rustok_core::{PermissionScope, SecurityContext, generate_id};
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
    TranslationTargetChangesRequest, TranslationTargetProgressFacts,
    TranslationTargetProgressRequest, TranslationTargetProvider,
    TranslationTargetProviderDescriptor, TranslationValueProfile,
    provider_support::{
        contract_validation_error, field_hash, merged_patch_values, read_request_from_patch,
        required_target_value, validate_patch_against_snapshot, validation_to_port_error,
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
const TRANSLATION_RESOURCE_KIND: &str = "variant_attribute_value";
const VALUE_FIELD: &str = "value";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_variant_attribute_value_patch";
const CHANGE_CURSOR_VERSION: &str = "v1";
const MAX_CHANGE_PAGE: u16 = 200;
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;
const LOCALE_REVISION_NAMESPACE: &str =
    "rustok-product/variant-attribute-value-translation-locale/v1";
const DELETED_REVISION_NAMESPACE: &str =
    "rustok-product/variant-attribute-value-translation-deleted/v1";
const RESOURCE_REVISION_PREFIX: &str = "product-variant-attribute-value-resource-v1:";

#[derive(Debug, Error)]
enum ProductVariantAttributeValueTranslationError {
    #[error(transparent)]
    Commerce(#[from] CommerceError),
    #[error("Product Variant localized attribute value resource not found: {0}")]
    ValueNotFound(Uuid),
    #[error(
        "Product Variant localized attribute value source locale not found: {locale} for value {value_id}"
    )]
    SourceLocaleNotFound { value_id: Uuid, locale: String },
    #[error("Product Variant localized attribute value {revision} revision conflict")]
    RevisionConflict { revision: &'static str },
    #[error("Product Variant localized attribute value owner invariant failed: {0}")]
    OwnerInvariant(String),
}

impl From<sea_orm::DbErr> for ProductVariantAttributeValueTranslationError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Commerce(CommerceError::Database(error))
    }
}

type OwnerResult<T> = Result<T, ProductVariantAttributeValueTranslationError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredTranslation {
    locale: String,
    value_text: Option<String>,
}

#[derive(Debug, Clone)]
struct Aggregate {
    id: Uuid,
    tenant_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
    attribute_id: Uuid,
    attribute_code: String,
    translation_revision: u64,
    translations: Vec<StoredTranslation>,
}

#[derive(Debug, FromQueryResult)]
struct AggregateRow {
    id: Uuid,
    tenant_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
    attribute_id: Uuid,
    attribute_code: String,
    translation_revision: i64,
    translations: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExactLocaleSnapshot {
    value_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
    attribute_id: Uuid,
    attribute_code: String,
    source_locale: String,
    target_locale: String,
    resource_revision: String,
    source_revision: String,
    target_revision: Option<String>,
    exact_locales: Vec<String>,
    source_value: String,
    target_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExactLocaleApply {
    source_locale: String,
    target_locale: String,
    value: String,
    expected_resource_revision: String,
    expected_source_revision: String,
    expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExactLocaleApplyReceipt {
    operation_id: Option<Uuid>,
    value_id: Uuid,
    resource_revision: String,
    target_revision: String,
    target_value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProgressFacts {
    resources: u64,
    exact_required_units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangeLifecycle {
    Active,
    Deleted,
}

impl ChangeLifecycle {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deleted => "deleted",
        }
    }

    fn parse(value: &str) -> OwnerResult<Self> {
        match value {
            "active" => Ok(Self::Active),
            "deleted" => Ok(Self::Deleted),
            _ => Err(
                ProductVariantAttributeValueTranslationError::OwnerInvariant(
                    "change journal returned an unknown lifecycle".to_string(),
                ),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChangeRecord {
    change_seq: u64,
    value_id: Uuid,
    resource_revision: String,
    lifecycle: ChangeLifecycle,
}

#[derive(Debug, FromQueryResult)]
struct LiveValueRow {
    value_id: Uuid,
    variant_id: Uuid,
    translation_revision: i64,
}

#[derive(Debug, FromQueryResult)]
struct PreviousValueRow {
    value_id: Uuid,
    variant_id: Uuid,
    resource_revision: String,
    lifecycle: String,
}

#[derive(Clone)]
struct ProductVariantAttributeValueTranslationTargetProvider {
    service: Arc<ProductCatalogSchemaService>,
}

impl ProductCatalogSchemaService {
    /// Builds the neutral Translation adapter for Product-owned localized Variant EAV values.
    ///
    /// One canonical `product_variant_attribute_values` row is one Translation resource. Dynamic
    /// category/form membership is context only and never changes the Translation identity.
    pub fn variant_attribute_value_translation_target_provider(
        service: Arc<Self>,
    ) -> impl TranslationTargetProvider {
        ProductVariantAttributeValueTranslationTargetProvider { service }
    }

    async fn list_variant_attribute_value_translation_exact_resources(
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
                "Product Variant attribute-value translation resource page size must be between 1 and 200"
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
        let next_after = has_more
            .then(|| aggregates.last().map(|row| row.id))
            .flatten();
        let resources = aggregates
            .into_iter()
            .map(|aggregate| build_snapshot(aggregate, &source_locale, &target_locale))
            .collect::<OwnerResult<Vec<_>>>()?;
        Ok((resources, next_after))
    }

    async fn read_variant_attribute_value_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        value_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> OwnerResult<ExactLocaleSnapshot> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_uuid(value_id, "value_id")?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        ensure_postgres(self.db.get_database_backend())?;
        let aggregate = load_variant_attribute_value_aggregate(&self.db, tenant_id, value_id)
            .await?
            .ok_or(ProductVariantAttributeValueTranslationError::ValueNotFound(
                value_id,
            ))?;
        build_snapshot(aggregate, &source_locale, &target_locale)
    }

    async fn apply_variant_attribute_value_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        value_id: Uuid,
        request: ExactLocaleApply,
    ) -> OwnerResult<ExactLocaleApplyReceipt> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_uuid(value_id, "value_id")?;
        if let Some(actor_user_id) = actor_user_id {
            validate_uuid(actor_user_id, "actor_user_id")?;
        }
        let source_locale = canonical_locale(&request.source_locale)?;
        let target_locale = canonical_locale(&request.target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        validate_target_value(&request.value)?;
        ensure_postgres(self.db.get_database_backend())?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        let locked = txn
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
SELECT pvav.id
FROM product_variant_attribute_values pvav
INNER JOIN product_attributes pa
    ON pa.id = pvav.attribute_id
   AND pa.tenant_id = pvav.tenant_id
INNER JOIN product_variants pv
    ON pv.id = pvav.variant_id
   AND pv.tenant_id = pvav.tenant_id
WHERE pvav.tenant_id = $1
  AND pvav.id = $2
  AND pa.is_localized = TRUE
  AND pa.value_type IN ('text', 'textarea', 'richtext')
FOR UPDATE OF pvav
"#,
                vec![tenant_id.into(), value_id.into()],
            ))
            .await?;
        if locked.is_none() {
            return Err(ProductVariantAttributeValueTranslationError::ValueNotFound(
                value_id,
            ));
        }

        let before = build_snapshot(
            load_variant_attribute_value_aggregate(&txn, tenant_id, value_id)
                .await?
                .ok_or(ProductVariantAttributeValueTranslationError::ValueNotFound(
                    value_id,
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
            return Err(
                ProductVariantAttributeValueTranslationError::RevisionConflict {
                    revision: "target",
                },
            );
        }

        txn.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
INSERT INTO product_variant_attribute_value_translations (id, value_id, locale, value_text)
VALUES ($1, $2, $3, $4)
ON CONFLICT (value_id, locale) DO UPDATE SET value_text = EXCLUDED.value_text
"#,
            vec![
                generate_id().into(),
                value_id.into(),
                target_locale.clone().into(),
                request.value.clone().into(),
            ],
        ))
        .await?;

        let after = build_snapshot(
            load_variant_attribute_value_aggregate(&txn, tenant_id, value_id)
                .await?
                .ok_or(ProductVariantAttributeValueTranslationError::ValueNotFound(
                    value_id,
                ))?,
            &source_locale,
            &target_locale,
        )?;
        let target_revision = after.target_revision.clone().ok_or_else(|| {
            ProductVariantAttributeValueTranslationError::OwnerInvariant(
                "exact target revision is absent after Product Variant attribute-value translation apply"
                    .to_string(),
            )
        })?;
        let target_value = after.target_value.clone().ok_or_else(|| {
            ProductVariantAttributeValueTranslationError::OwnerInvariant(
                "exact target value is absent after Product Variant attribute-value translation apply"
                    .to_string(),
            )
        })?;

        let touch = txn
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "UPDATE product_variants SET updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND id = $2 AND product_id = $3",
                vec![
                    tenant_id.into(),
                    after.variant_id.into(),
                    after.product_id.into(),
                ],
            ))
            .await?;
        if touch.rows_affected() != 1 {
            return Err(CommerceError::VariantNotFound(after.variant_id).into());
        }

        txn.publish(
            tenant_id,
            actor_user_id,
            DomainEvent::VariantUpdated {
                variant_id: after.variant_id,
                product_id: after.product_id,
            },
        )
        .await?;
        let receipt = ExactLocaleApplyReceipt {
            operation_id: current_product_operation_id(),
            value_id,
            resource_revision: after.resource_revision,
            target_revision,
            target_value,
        };
        record_product_operation_result(&receipt)?;
        txn.commit().await?;
        Ok(receipt)
    }

    async fn read_variant_attribute_value_translation_progress(
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
    COUNT(*) AS resources,
    COUNT(*) FILTER (
        WHERE NULLIF(BTRIM(target_translation.value_text), '') IS NOT NULL
    ) AS exact_required_units
FROM product_variant_attribute_values pvav
INNER JOIN product_attributes pa
    ON pa.id = pvav.attribute_id
   AND pa.tenant_id = pvav.tenant_id
INNER JOIN product_variants pv
    ON pv.id = pvav.variant_id
   AND pv.tenant_id = pvav.tenant_id
INNER JOIN product_variant_attribute_value_translations source_translation
    ON source_translation.value_id = pvav.id
   AND source_translation.locale = $2
LEFT JOIN product_variant_attribute_value_translations target_translation
    ON target_translation.value_id = pvav.id
   AND target_translation.locale = $3
WHERE pvav.tenant_id = $1
  AND pa.is_localized = TRUE
  AND pa.value_type IN ('text', 'textarea', 'richtext')
  AND NULLIF(BTRIM(source_translation.value_text), '') IS NOT NULL
"#,
                vec![tenant_id.into(), source_locale.into(), target_locale.into()],
            ))
            .await?
            .ok_or_else(|| {
                ProductVariantAttributeValueTranslationError::OwnerInvariant(
                    "progress aggregate query returned no row".to_string(),
                )
            })?;
        let resources = nonnegative_count(row.try_get("", "resources")?, "resources")?;
        let exact_required_units = nonnegative_count(
            row.try_get("", "exact_required_units")?,
            "exact_required_units",
        )?;
        txn.commit().await?;
        Ok(ProgressFacts {
            resources,
            exact_required_units,
        })
    }

    async fn variant_attribute_value_translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> OwnerResult<Option<u64>> {
        validate_uuid(tenant_id, "tenant_id")?;
        ensure_postgres(self.db.get_database_backend())?;
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT MAX(change_seq) AS highwater FROM product_variant_attribute_value_translation_change_journal WHERE tenant_id = $1",
                vec![tenant_id.into()],
            ))
            .await?
            .ok_or_else(|| {
                ProductVariantAttributeValueTranslationError::OwnerInvariant(
                    "change high-water query returned no row".to_string(),
                )
            })?;
        optional_positive_sequence(row.try_get("", "highwater")?, "high-water")
    }

    async fn read_variant_attribute_value_translation_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> OwnerResult<Vec<ChangeRecord>> {
        validate_uuid(tenant_id, "tenant_id")?;
        if through_seq == 0 || after_seq > through_seq {
            return Err(CommerceError::Validation(
                "Product Variant attribute-value translation change cursor bounds are invalid"
                    .to_string(),
            )
            .into());
        }
        if limit == 0 || limit > MAX_CHANGE_PAGE {
            return Err(CommerceError::Validation(format!(
                "Product Variant attribute-value translation change page size must be between 1 and {MAX_CHANGE_PAGE}"
            ))
            .into());
        }
        ensure_postgres(self.db.get_database_backend())?;
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
SELECT change_seq, value_id, resource_revision, lifecycle
FROM product_variant_attribute_value_translation_change_journal
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

    pub(crate) async fn record_variant_attribute_value_translation_changes_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        product_id: Uuid,
        root_event_id: Uuid,
        variant_id: Option<Uuid>,
    ) -> CommerceResult<()> {
        if txn.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }
        if tenant_id.is_nil()
            || product_id.is_nil()
            || root_event_id.is_nil()
            || variant_id.is_some_and(|variant_id| variant_id.is_nil())
        {
            return Err(CommerceError::Validation(
                "Product Variant attribute-value translation change identity must not be nil"
                    .to_string(),
            ));
        }

        let live_rows = LiveValueRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
SELECT pvav.id AS value_id, pvav.variant_id, pvav.translation_revision
FROM product_variant_attribute_values pvav
INNER JOIN product_variants pv
    ON pv.id = pvav.variant_id
   AND pv.tenant_id = pvav.tenant_id
INNER JOIN product_attributes pa
    ON pa.id = pvav.attribute_id
   AND pa.tenant_id = pvav.tenant_id
WHERE pvav.tenant_id = $1
  AND pv.product_id = $2
  AND ($3::uuid IS NULL OR pvav.variant_id = $3)
  AND pa.is_localized = TRUE
  AND pa.value_type IN ('text', 'textarea', 'richtext')
  AND EXISTS (
      SELECT 1
      FROM product_variant_attribute_value_translations pvavt
      WHERE pvavt.value_id = pvav.id
  )
ORDER BY pvav.id
"#,
            vec![tenant_id.into(), product_id.into(), variant_id.into()],
        ))
        .all(txn)
        .await?;
        let previous_rows = PreviousValueRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
SELECT DISTINCT ON (value_id)
    value_id,
    variant_id,
    resource_revision,
    lifecycle
FROM product_variant_attribute_value_translation_change_journal
WHERE tenant_id = $1
  AND product_id = $2
  AND ($3::uuid IS NULL OR variant_id = $3)
ORDER BY value_id, change_seq DESC
"#,
            vec![tenant_id.into(), product_id.into(), variant_id.into()],
        ))
        .all(txn)
        .await?;
        let previous = previous_rows
            .into_iter()
            .map(|row| (row.value_id, row))
            .collect::<HashMap<_, _>>();
        let live_ids = live_rows
            .iter()
            .map(|row| row.value_id)
            .collect::<BTreeSet<_>>();

        for row in live_rows {
            let revision = resource_revision_from_sequence(row.translation_revision)
                .map_err(owner_error_to_commerce)?;
            if previous.get(&row.value_id).is_some_and(|previous| {
                previous.lifecycle == ChangeLifecycle::Active.as_str()
                    && previous.resource_revision == revision
            }) {
                continue;
            }
            insert_change(
                txn,
                root_event_id,
                tenant_id,
                product_id,
                row.variant_id,
                row.value_id,
                &revision,
                ChangeLifecycle::Active,
            )
            .await?;
        }

        for (value_id, previous) in previous {
            if live_ids.contains(&value_id)
                || previous.lifecycle == ChangeLifecycle::Deleted.as_str()
            {
                continue;
            }
            let revision = deleted_revision(root_event_id, value_id);
            insert_change(
                txn,
                root_event_id,
                tenant_id,
                product_id,
                previous.variant_id,
                value_id,
                &revision,
                ChangeLifecycle::Deleted,
            )
            .await?;
        }
        Ok(())
    }
}

#[async_trait]
impl TranslationTargetProvider for ProductVariantAttributeValueTranslationTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Product owner slug must satisfy Translation contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND).expect(
                "static Product Variant Attribute Value resource kind must satisfy Translation contract",
            ),
            display_name: "Product Variant localized attribute values".to_string(),
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
            .list_variant_attribute_value_translation_exact_resources(
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
                .variant_attribute_value_translation_change_highwater(tenant_id)
                .await
                .map_err(owner_error_to_port_error)?;
            let owner = self
                .service
                .read_variant_attribute_value_translation_progress(
                    tenant_id,
                    request.source_locale.as_str(),
                    request.target_locale.as_str(),
                )
                .await
                .map_err(owner_error_to_port_error)?;
            let after = self
                .service
                .variant_attribute_value_translation_change_highwater(tenant_id)
                .await
                .map_err(owner_error_to_port_error)?;
            if before != after {
                continue;
            }
            let facts = TranslationTargetProgressFacts {
                required_units: owner.resources,
                exact_required_units: owner.exact_required_units,
                optional_units: 0,
                exact_optional_units: 0,
                resources: owner.resources,
                complete_resources: owner.exact_required_units,
                owner_change_cursor: after
                    .map(|change_seq| change_cursor(change_seq, change_seq))
                    .transpose()?,
            };
            facts.validate().map_err(|error| {
                PortError::invariant_violation(
                    "product.variant_attribute_value_translation_progress_invalid",
                    error.to_string(),
                )
            })?;
            return Ok(facts);
        }
        Err(PortError::unavailable(
            "product.variant_attribute_value_translation_progress_unstable",
            "Product Variant attribute-value translation progress changed while it was being aggregated",
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
        let parsed = request
            .after
            .as_ref()
            .map(parse_change_cursor)
            .transpose()?;
        let (through, after) = match parsed {
            Some((through, after)) if through == after => {
                let current = self
                    .service
                    .variant_attribute_value_translation_change_highwater(tenant_id)
                    .await
                    .map_err(owner_error_to_port_error)?
                    .unwrap_or(after)
                    .max(after);
                (current, after)
            }
            Some(cursor) => cursor,
            None => (
                self.service
                    .variant_attribute_value_translation_change_highwater(tenant_id)
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
            .read_variant_attribute_value_translation_changes(
                tenant_id,
                after,
                through,
                request.limit,
            )
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
                    identity: variant_attribute_value_identity(change.value_id),
                    resource_revision: opaque_revision(
                        change.resource_revision,
                        "resource_revision",
                    )?,
                    lifecycle: match change.lifecycle {
                        ChangeLifecycle::Active => TranslationResourceLifecycle::Active,
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
        Ok(validate_patch_against_snapshot(&request, &snapshot))
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
        let value_id = parse_identity(&request.identity)?;
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
                self.service
                    .apply_variant_attribute_value_translation_exact_locale(
                        tenant_id,
                        security.user_id,
                        value_id,
                        target,
                    ),
            )
            .await
            .map_err(owner_error_to_port_error)?;
            if applied.operation_id != Some(lease.operation_id) {
                return Err(PortError::invariant_violation(
                    "product.variant_attribute_value_translation_receipt_identity_invalid",
                    "Product Variant attribute-value owner receipt is not bound to the active Translation operation",
                ));
            }
            application_receipt(&applied, &request)
        }
        .await;
        if let Err(error) = &result {
            self.service
                .fail_schema_operation_receipt(lease, error)
                .await?;
        }
        result
    }
}

async fn load_owner_snapshot(
    service: &ProductCatalogSchemaService,
    tenant_id: Uuid,
    request: &ReadTranslationResourceRequest,
) -> Result<ExactLocaleSnapshot, PortError> {
    let value_id = parse_identity(&request.identity)?;
    service
        .read_variant_attribute_value_translation_exact_locale(
            tenant_id,
            value_id,
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

async fn load_variant_attribute_value_aggregate<C>(
    db: &C,
    tenant_id: Uuid,
    value_id: Uuid,
) -> OwnerResult<Option<Aggregate>>
where
    C: ConnectionTrait,
{
    AggregateRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!(
            "{} WHERE pvav.tenant_id = $1 AND pvav.id = $2",
            aggregate_select_sql()
        ),
        vec![tenant_id.into(), value_id.into()],
    ))
    .one(db)
    .await?
    .map(decode_aggregate)
    .transpose()
}

fn aggregate_select_sql() -> &'static str {
    r#"
SELECT
    pvav.id,
    pvav.tenant_id,
    pv.product_id,
    pvav.variant_id,
    pvav.attribute_id,
    pa.code AS attribute_code,
    pvav.translation_revision,
    COALESCE((
        SELECT jsonb_agg(
            jsonb_build_object(
                'locale', pvavt.locale,
                'value_text', pvavt.value_text
            ) ORDER BY pvavt.locale
        )
        FROM product_variant_attribute_value_translations pvavt
        WHERE pvavt.value_id = pvav.id
    ), '[]'::jsonb) AS translations
FROM product_variant_attribute_values pvav
INNER JOIN product_variants pv
    ON pv.id = pvav.variant_id
   AND pv.tenant_id = pvav.tenant_id
INNER JOIN product_attributes pa
    ON pa.id = pvav.attribute_id
   AND pa.tenant_id = pvav.tenant_id
   AND pa.is_localized = TRUE
   AND pa.value_type IN ('text', 'textarea', 'richtext')
"#
}

fn source_eligible_sql(with_after: bool) -> String {
    let after = if with_after { "AND pvav.id > $3" } else { "" };
    let limit = if with_after { "$4" } else { "$3" };
    format!(
        r#"
WITH candidate AS (
    SELECT pvav.id
    FROM product_variant_attribute_values pvav
    INNER JOIN product_variants pv
        ON pv.id = pvav.variant_id
       AND pv.tenant_id = pvav.tenant_id
    INNER JOIN product_attributes pa
        ON pa.id = pvav.attribute_id
       AND pa.tenant_id = pvav.tenant_id
    INNER JOIN product_variant_attribute_value_translations source_translation
        ON source_translation.value_id = pvav.id
       AND source_translation.locale = $2
    WHERE pvav.tenant_id = $1
      AND pa.is_localized = TRUE
      AND pa.value_type IN ('text', 'textarea', 'richtext')
      AND NULLIF(BTRIM(source_translation.value_text), '') IS NOT NULL
      {after}
    ORDER BY pvav.id ASC
    LIMIT {limit}
)
{}
INNER JOIN candidate ON candidate.id = pvav.id
ORDER BY pvav.id ASC
"#,
        aggregate_select_sql()
    )
}

fn decode_aggregate(row: AggregateRow) -> OwnerResult<Aggregate> {
    let translations: Vec<StoredTranslation> =
        serde_json::from_value(row.translations).map_err(|error| {
            ProductVariantAttributeValueTranslationError::OwnerInvariant(format!(
                "persisted Product Variant attribute-value translations are invalid: {error}"
            ))
        })?;
    if row.id.is_nil()
        || row.tenant_id.is_nil()
        || row.product_id.is_nil()
        || row.variant_id.is_nil()
        || row.attribute_id.is_nil()
        || row.attribute_code.trim().is_empty()
    {
        return Err(
            ProductVariantAttributeValueTranslationError::OwnerInvariant(
                "persisted Product Variant attribute-value identity is invalid".to_string(),
            ),
        );
    }
    let translation_revision = positive_sequence(row.translation_revision, "resource")?;
    for translation in &translations {
        validate_stored_translation(translation)?;
    }
    Ok(Aggregate {
        id: row.id,
        tenant_id: row.tenant_id,
        product_id: row.product_id,
        variant_id: row.variant_id,
        attribute_id: row.attribute_id,
        attribute_code: row.attribute_code,
        translation_revision,
        translations,
    })
}

fn build_snapshot(
    aggregate: Aggregate,
    source_locale: &str,
    target_locale: &str,
) -> OwnerResult<ExactLocaleSnapshot> {
    let source = aggregate
        .translations
        .iter()
        .find(|translation| translation.locale == source_locale)
        .and_then(|translation| translation.value_text.clone())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(
            || ProductVariantAttributeValueTranslationError::SourceLocaleNotFound {
                value_id: aggregate.id,
                locale: source_locale.to_string(),
            },
        )?;
    let target_record = aggregate
        .translations
        .iter()
        .find(|translation| translation.locale == target_locale);
    let target_value = target_record.and_then(|translation| translation.value_text.clone());
    let target_revision = target_record.map(|translation| {
        locale_revision(
            aggregate.tenant_id,
            aggregate.id,
            target_locale,
            translation.value_text.as_deref(),
        )
    });
    let exact_locales = aggregate
        .translations
        .iter()
        .filter_map(|translation| {
            translation
                .value_text
                .as_ref()
                .filter(|value| !value.trim().is_empty())
                .map(|_| translation.locale.clone())
        })
        .collect::<Vec<_>>();
    Ok(ExactLocaleSnapshot {
        value_id: aggregate.id,
        product_id: aggregate.product_id,
        variant_id: aggregate.variant_id,
        attribute_id: aggregate.attribute_id,
        attribute_code: aggregate.attribute_code,
        source_locale: source_locale.to_string(),
        target_locale: target_locale.to_string(),
        resource_revision: resource_revision(aggregate.translation_revision),
        source_revision: locale_revision(
            aggregate.tenant_id,
            aggregate.id,
            source_locale,
            Some(source.as_str()),
        ),
        target_revision,
        exact_locales,
        source_value: source,
        target_value,
    })
}

fn resource_revision(sequence: u64) -> String {
    format!("{RESOURCE_REVISION_PREFIX}{sequence}")
}

fn resource_revision_from_sequence(sequence: i64) -> OwnerResult<String> {
    positive_sequence(sequence, "resource").map(resource_revision)
}

fn locale_revision(tenant_id: Uuid, value_id: Uuid, locale: &str, value: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hash_str(&mut hasher, LOCALE_REVISION_NAMESPACE);
    hash_uuid(&mut hasher, tenant_id);
    hash_uuid(&mut hasher, value_id);
    hash_str(&mut hasher, locale);
    hash_optional_str(&mut hasher, value);
    format!(
        "product-variant-attribute-value-locale-v1:{}",
        hex::encode(hasher.finalize())
    )
}

fn deleted_revision(root_event_id: Uuid, value_id: Uuid) -> String {
    let mut hasher = Sha256::new();
    hash_str(&mut hasher, DELETED_REVISION_NAMESPACE);
    hash_uuid(&mut hasher, root_event_id);
    hash_uuid(&mut hasher, value_id);
    format!(
        "product-variant-attribute-value-deleted-v1:{}",
        hex::encode(hasher.finalize())
    )
}

fn summary_from_owner(
    owner: &ExactLocaleSnapshot,
) -> Result<TranslationResourceSummary, PortError> {
    Ok(TranslationResourceSummary {
        identity: variant_attribute_value_identity(owner.value_id),
        display_label: format!("{} · {}", owner.attribute_code, owner.variant_id),
        lifecycle: TranslationResourceLifecycle::Active,
        resource_revision: opaque_revision(owner.resource_revision.clone(), "resource_revision")?,
        exact_locales: owner
            .exact_locales
            .iter()
            .map(|locale| {
                TenantLocale::new(locale.clone()).map_err(|error| {
                    PortError::invariant_violation(
                        "product.variant_attribute_value_translation_locale_invalid",
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
            "product.variant_attribute_value_translation_locale_identity_invalid",
            "Product Variant attribute-value owner returned different exact locales",
        ));
    }
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
        fields: vec![TranslationFieldSnapshot {
            descriptor: TranslationFieldDescriptor {
                key: FieldKey::new(VALUE_FIELD).expect("static Product value field must be valid"),
                profile: TranslationValueProfile::PlainText,
                strategy: TranslationStrategy::Translate,
                classification: TranslationDataClassification::TenantPrivate,
                required: true,
                ai_export_allowed: false,
                max_characters: None,
                preserves_whitespace: false,
            },
            source_value: owner.source_value.clone(),
            exact_target_value: owner.target_value.clone(),
            source_hash: field_hash(&owner.source_value),
            protected_tokens: Vec::new(),
        }],
    };
    snapshot.validate().map_err(|error| {
        PortError::invariant_violation(
            "product.variant_attribute_value_translation_snapshot_invalid",
            error.to_string(),
        )
    })?;
    Ok(snapshot)
}

fn merged_target(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> Result<ExactLocaleApply, PortError> {
    let mut values = merged_patch_values(request, snapshot);
    let value = required_target_value(values.remove(VALUE_FIELD).flatten(), "attribute value")?;
    if !values.is_empty() {
        return Err(PortError::validation(
            "product.variant_attribute_value_translation_patch_field_unknown",
            "Product Variant attribute-value translation patch contains an unknown field",
        ));
    }
    Ok(ExactLocaleApply {
        source_locale: request.source_locale.as_str().to_string(),
        target_locale: request.target_locale.as_str().to_string(),
        value,
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
            "product.variant_attribute_value_translation_receipt_identity_missing",
            "Product Variant attribute-value owner receipt is missing operation identity",
        )
    })?;
    Ok(TranslationApplicationReceipt {
        provider_receipt_id: format!("product-variant-attribute-value:{operation_id}"),
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
    product_id: Uuid,
    variant_id: Uuid,
    value_id: Uuid,
    resource_revision: &str,
    lifecycle: ChangeLifecycle,
) -> CommerceResult<()> {
    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
INSERT INTO product_variant_attribute_value_translation_change_journal (
    root_event_id, tenant_id, product_id, variant_id, value_id, resource_revision, lifecycle, created_at
) VALUES ($1, $2, $3, $4, $5, $6, $7, CURRENT_TIMESTAMP)
ON CONFLICT (root_event_id, value_id) DO NOTHING
"#,
        vec![
            root_event_id.into(),
            tenant_id.into(),
            product_id.into(),
            variant_id.into(),
            value_id.into(),
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
            "Product Variant attribute-value translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Products, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "product.variant_attribute_value_translation_permission_denied",
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
            "product.variant_attribute_value_translation_identity_invalid",
            "Product Variant attribute-value identity must address product/variant_attribute_value without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "product.variant_attribute_value_translation_resource_id_invalid",
            "Product Variant attribute-value resource id must be a UUID",
        )
    })
}

fn variant_attribute_value_identity(value_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Product owner slug must satisfy Translation contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND).expect(
            "static Product Variant Attribute Value resource kind must satisfy Translation contract",
        ),
        resource_id: ResourceId::new(value_id.to_string()).expect(
            "Product Variant attribute-value UUID must satisfy Translation resource id contract",
        ),
        subresource_id: None,
    }
}

fn resource_cursor(value_id: Uuid) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(value_id.to_string()).map_err(|error| {
        PortError::invariant_violation(
            "product.variant_attribute_value_translation_resource_cursor_invalid",
            error.to_string(),
        )
    })
}

fn parse_resource_cursor(value: &str) -> Result<Uuid, PortError> {
    Uuid::parse_str(value).map_err(|_| {
        PortError::validation(
            "product.variant_attribute_value_translation_resource_cursor_invalid",
            "Product Variant attribute-value resource cursor must be a UUID",
        )
    })
}

fn change_cursor(through: u64, after: u64) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(format!("{CHANGE_CURSOR_VERSION}:{through}:{after}")).map_err(|error| {
        PortError::invariant_violation(
            "product.variant_attribute_value_translation_change_cursor_invalid",
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
            "product.variant_attribute_value_translation_change_cursor_invalid",
            "Product Variant attribute-value translation change cursor is invalid",
        ));
    }
    let through = through.unwrap_or_default();
    let after = after.unwrap_or_default();
    if through == 0 || after == 0 || after > through {
        return Err(PortError::validation(
            "product.variant_attribute_value_translation_change_cursor_invalid",
            "Product Variant attribute-value translation change cursor bounds are invalid",
        ));
    }
    Ok((through, after))
}

fn opaque_revision(value: String, field: &'static str) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(value).map_err(|error| {
        PortError::invariant_violation(
            "product.variant_attribute_value_translation_revision_invalid",
            format!("Product Variant attribute-value {field} is invalid: {error}"),
        )
    })
}

fn owner_error_to_port_error(error: ProductVariantAttributeValueTranslationError) -> PortError {
    match error {
        ProductVariantAttributeValueTranslationError::ValueNotFound(_) => PortError::not_found(
            "product.variant_attribute_value_translation_resource_not_found",
            "Product Variant attribute-value translation resource was not found",
        ),
        ProductVariantAttributeValueTranslationError::SourceLocaleNotFound { .. } => {
            PortError::not_found(
                "product.variant_attribute_value_translation_source_not_found",
                "Exact source Product Variant attribute-value locale was not found",
            )
        }
        ProductVariantAttributeValueTranslationError::RevisionConflict { .. } => {
            PortError::conflict(
                "product.variant_attribute_value_translation_revision_conflict",
                "Product Variant attribute-value translation state conflicts with the requested mutation",
            )
        }
        ProductVariantAttributeValueTranslationError::OwnerInvariant(_) => {
            PortError::invariant_violation(
                "product.variant_attribute_value_translation_owner_invariant",
                "Product Variant attribute-value translation owner state is invalid",
            )
        }
        ProductVariantAttributeValueTranslationError::Commerce(error) => {
            product_error_to_port_error(error)
        }
    }
}

fn product_error_to_port_error(error: CommerceError) -> PortError {
    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "product.variant_attribute_value_translation_owner_unavailable",
            "Product Variant attribute-value translation storage is temporarily unavailable",
        ),
        CommerceError::ProductNotFound(_)
        | CommerceError::VariantNotFound(_)
        | CommerceError::ImageNotFound(_) => PortError::not_found(
            "product.variant_attribute_value_translation_resource_not_found",
            "Product Variant attribute-value translation resource was not found",
        ),
        CommerceError::DuplicateHandle { .. }
        | CommerceError::DuplicateSku(_)
        | CommerceError::CannotDeleteOnlyVariant
        | CommerceError::CannotDeletePublished => PortError::conflict(
            "product.variant_attribute_value_translation_owner_conflict",
            "Product state conflicts with the requested Variant attribute-value translation mutation",
        ),
        CommerceError::Validation(_) | CommerceError::NoVariants => PortError::validation(
            "product.variant_attribute_value_translation_owner_validation",
            "Product rejected the Variant attribute-value translation mutation",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "product.variant_attribute_value_translation_owner_invariant",
            "Product Variant attribute-value translation state is invalid",
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
            "Product Variant attribute-value translation source and target locale must differ"
                .to_string(),
        )
        .into());
    }
    Ok(())
}

fn validate_stored_translation(translation: &StoredTranslation) -> OwnerResult<()> {
    if canonical_locale(&translation.locale)? != translation.locale {
        return Err(
            ProductVariantAttributeValueTranslationError::OwnerInvariant(
                "persisted Product Variant attribute-value translation locale is not canonical"
                    .to_string(),
            ),
        );
    }
    Ok(())
}

fn validate_target_value(value: &str) -> OwnerResult<()> {
    if value.trim().is_empty() {
        return Err(CommerceError::Validation(
            "Product Variant attribute-value translation target must not be blank".to_string(),
        )
        .into());
    }
    Ok(())
}

fn validate_uuid(value: Uuid, field: &str) -> OwnerResult<()> {
    if value.is_nil() {
        return Err(CommerceError::Validation(format!(
            "Product Variant attribute-value translation {field} must not be nil"
        ))
        .into());
    }
    Ok(())
}

fn ensure_revision(revision: &'static str, expected: &str, current: &str) -> OwnerResult<()> {
    if expected != current {
        return Err(ProductVariantAttributeValueTranslationError::RevisionConflict { revision });
    }
    Ok(())
}

fn ensure_postgres(backend: DatabaseBackend) -> OwnerResult<()> {
    if backend != DatabaseBackend::Postgres {
        return Err(CommerceError::Validation(
            "Product Variant attribute-value Translation target requires PostgreSQL".to_string(),
        )
        .into());
    }
    Ok(())
}

fn nonnegative_count(value: i64, field: &str) -> OwnerResult<u64> {
    u64::try_from(value).map_err(|_| {
        ProductVariantAttributeValueTranslationError::OwnerInvariant(format!(
            "Product Variant attribute-value translation {field} count must be nonnegative"
        ))
    })
}

fn optional_positive_sequence(value: Option<i64>, field: &str) -> OwnerResult<Option<u64>> {
    value
        .map(|value| positive_sequence(value, field))
        .transpose()
}

fn positive_sequence(value: i64, field: &str) -> OwnerResult<u64> {
    let value = u64::try_from(value).map_err(|_| invalid_sequence(field))?;
    if value == 0 {
        return Err(invalid_sequence(field));
    }
    Ok(value)
}

fn invalid_sequence(field: &str) -> ProductVariantAttributeValueTranslationError {
    ProductVariantAttributeValueTranslationError::OwnerInvariant(format!(
        "Product Variant attribute-value translation change {field} sequence must be positive"
    ))
}

fn change_record_from_row(row: sea_orm::QueryResult) -> OwnerResult<ChangeRecord> {
    let change_seq = positive_sequence(row.try_get("", "change_seq")?, "change")?;
    let value_id: Uuid = row.try_get("", "value_id")?;
    let resource_revision: String = row.try_get("", "resource_revision")?;
    let lifecycle: String = row.try_get("", "lifecycle")?;
    if value_id.is_nil() || resource_revision.trim().is_empty() {
        return Err(
            ProductVariantAttributeValueTranslationError::OwnerInvariant(
                "Product Variant attribute-value translation change row is invalid".to_string(),
            ),
        );
    }
    Ok(ChangeRecord {
        change_seq,
        value_id,
        resource_revision,
        lifecycle: ChangeLifecycle::parse(&lifecycle)?,
    })
}

fn owner_error_to_commerce(error: ProductVariantAttributeValueTranslationError) -> CommerceError {
    match error {
        ProductVariantAttributeValueTranslationError::Commerce(error) => error,
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
