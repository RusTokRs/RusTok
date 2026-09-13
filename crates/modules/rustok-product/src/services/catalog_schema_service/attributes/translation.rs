use std::{collections::BTreeSet, sync::Arc};

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
    TranslationTargetChangesRequest, TranslationTargetProgressFacts, TranslationTargetProgressRequest,
    TranslationTargetProvider, TranslationTargetProviderDescriptor, TranslationValueProfile,
    provider_support::{
        contract_validation_error, field_hash, merged_patch_values, normalize_optional_target_value,
        read_request_from_patch, required_target_value, validate_patch_against_snapshot,
        validation_to_port_error,
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
const TRANSLATION_RESOURCE_KIND: &str = "attribute";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_attribute_patch";
const ATTRIBUTE_LABEL_FIELD: &str = "label";
const ATTRIBUTE_HELP_TEXT_FIELD: &str = "help_text";
const ATTRIBUTE_FACET_LABEL_FIELD: &str = "facet_label";
const ATTRIBUTE_SEO_LABEL_FIELD: &str = "seo_label";
const OPTION_LABEL_PREFIX: &str = "option:";
const CHANGE_CURSOR_VERSION: &str = "v1";
const MAX_CHANGE_PAGE: u16 = 200;
const PROGRESS_PAGE_SIZE: u16 = 200;
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;
const RESOURCE_REVISION_NAMESPACE: &str = "rustok-product/attribute-translation-resource/v1";
const LOCALE_REVISION_NAMESPACE: &str = "rustok-product/attribute-translation-locale/v1";
const DELETED_REVISION_NAMESPACE: &str = "rustok-product/attribute-translation-deleted/v1";

#[derive(Debug, Error)]
enum ProductAttributeTranslationError {
    #[error(transparent)]
    Commerce(#[from] CommerceError),
    #[error("Product attribute not found: {0}")]
    AttributeNotFound(Uuid),
    #[error("Product attribute translation source locale not found: {locale} for attribute {attribute_id}")]
    SourceLocaleNotFound { attribute_id: Uuid, locale: String },
    #[error("Product attribute translation locale is incomplete: {locale} for attribute {attribute_id}")]
    IncompleteLocale { attribute_id: Uuid, locale: String },
    #[error("Product attribute translation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },
    #[error("Product attribute translation option set changed while the mutation was prepared")]
    OptionSetMismatch,
    #[error("Product attribute translation owner invariant failed: {0}")]
    OwnerInvariant(String),
}

impl From<sea_orm::DbErr> for ProductAttributeTranslationError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Commerce(CommerceError::Database(error))
    }
}

type OwnerResult<T> = Result<T, ProductAttributeTranslationError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct LocaleOptionRecord {
    option_id: Uuid,
    position: i32,
    label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct LocaleRecord {
    locale: String,
    label: Option<String>,
    help_text: Option<String>,
    facet_label: Option<String>,
    seo_label: Option<String>,
    options: Vec<LocaleOptionRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExactLocaleSnapshot {
    attribute_id: Uuid,
    archived: bool,
    show_on_storefront: bool,
    source_locale: String,
    target_locale: String,
    resource_revision: String,
    source_revision: String,
    target_revision: Option<String>,
    exact_locales: Vec<String>,
    source: LocaleRecord,
    target: Option<LocaleRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OptionApply {
    option_id: Uuid,
    label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExactLocaleApply {
    source_locale: String,
    target_locale: String,
    label: String,
    help_text: Option<String>,
    facet_label: Option<String>,
    seo_label: Option<String>,
    options: Vec<OptionApply>,
    expected_resource_revision: String,
    expected_source_revision: String,
    expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExactLocaleApplyReceipt {
    operation_id: Option<Uuid>,
    attribute_id: Uuid,
    resource_revision: String,
    target_revision: String,
    target: LocaleRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProgressFacts {
    resources: u64,
    required_units: u64,
    exact_required_units: u64,
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
            _ => Err(ProductAttributeTranslationError::OwnerInvariant(
                "change journal returned an unknown lifecycle".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChangeRecord {
    change_seq: u64,
    attribute_id: Uuid,
    resource_revision: String,
    lifecycle: ChangeLifecycle,
}

#[derive(Debug, FromQueryResult)]
struct AggregateRow {
    id: Uuid,
    tenant_id: Uuid,
    archived: bool,
    show_on_storefront: bool,
    translations: JsonValue,
    options: JsonValue,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredTranslation {
    locale: String,
    label: String,
    help_text: Option<String>,
    facet_label: Option<String>,
    seo_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredOptionTranslation {
    locale: String,
    label: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredOption {
    id: Uuid,
    position: i32,
    translations: Vec<StoredOptionTranslation>,
}

#[derive(Debug, Clone)]
struct Aggregate {
    id: Uuid,
    tenant_id: Uuid,
    archived: bool,
    show_on_storefront: bool,
    translations: Vec<StoredTranslation>,
    options: Vec<StoredOption>,
}

#[derive(Clone)]
struct ProductAttributeTranslationTargetProvider {
    service: Arc<ProductCatalogSchemaService>,
}

impl ProductCatalogSchemaService {
    /// Builds the neutral Translation adapter over the canonical Product attribute owner.
    ///
    /// The concrete adapter remains private to Product so callers cannot bypass the owner service.
    pub fn attribute_translation_target_provider(
        service: Arc<Self>,
    ) -> impl TranslationTargetProvider {
        ProductAttributeTranslationTargetProvider { service }
    }

    async fn list_attribute_translation_exact_resources(
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
                "Product attribute translation resource page size must be between 1 and 200"
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

    async fn read_attribute_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        attribute_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> OwnerResult<ExactLocaleSnapshot> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_uuid(attribute_id, "attribute_id")?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        ensure_postgres(self.db.get_database_backend())?;
        let aggregate = load_attribute_aggregate(&self.db, tenant_id, attribute_id)
            .await?
            .ok_or(ProductAttributeTranslationError::AttributeNotFound(
                attribute_id,
            ))?;
        build_snapshot(aggregate, &source_locale, &target_locale)
    }

    async fn apply_attribute_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        attribute_id: Uuid,
        request: ExactLocaleApply,
    ) -> OwnerResult<ExactLocaleApplyReceipt> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_uuid(attribute_id, "attribute_id")?;
        if let Some(actor_user_id) = actor_user_id {
            validate_uuid(actor_user_id, "actor_user_id")?;
        }
        validate_target_value(&request.label, "label", Some(255))?;
        validate_optional_target_value(request.help_text.as_deref(), "help_text", None)?;
        validate_optional_target_value(request.facet_label.as_deref(), "facet_label", Some(255))?;
        validate_optional_target_value(request.seo_label.as_deref(), "seo_label", Some(255))?;
        let source_locale = canonical_locale(&request.source_locale)?;
        let target_locale = canonical_locale(&request.target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        let mut requested_option_ids = BTreeSet::new();
        for option in &request.options {
            validate_uuid(option.option_id, "option_id")?;
            validate_target_value(&option.label, "option label", Some(255))?;
            if !requested_option_ids.insert(option.option_id) {
                return Err(ProductAttributeTranslationError::OptionSetMismatch);
            }
        }
        ensure_postgres(self.db.get_database_backend())?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        let locked = txn
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT id FROM product_attributes WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
                vec![tenant_id.into(), attribute_id.into()],
            ))
            .await?;
        if locked.is_none() {
            return Err(ProductAttributeTranslationError::AttributeNotFound(
                attribute_id,
            ));
        }
        txn.query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT id FROM product_attribute_options WHERE tenant_id = $1 AND attribute_id = $2 ORDER BY id FOR UPDATE",
            vec![tenant_id.into(), attribute_id.into()],
        ))
        .await?;

        let before = build_snapshot(
            load_attribute_aggregate(&txn, tenant_id, attribute_id)
                .await?
                .ok_or(ProductAttributeTranslationError::AttributeNotFound(
                    attribute_id,
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
            return Err(ProductAttributeTranslationError::RevisionConflict {
                revision: "target",
            });
        }
        let current_option_ids = before
            .source
            .options
            .iter()
            .map(|option| option.option_id)
            .collect::<BTreeSet<_>>();
        if current_option_ids != requested_option_ids {
            return Err(ProductAttributeTranslationError::OptionSetMismatch);
        }

        txn.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
INSERT INTO product_attribute_translations (
    id, attribute_id, locale, label, help_text, facet_label, seo_label
) VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (attribute_id, locale) DO UPDATE SET
    label = EXCLUDED.label,
    help_text = EXCLUDED.help_text,
    facet_label = EXCLUDED.facet_label,
    seo_label = EXCLUDED.seo_label
"#,
            vec![
                generate_id().into(),
                attribute_id.into(),
                target_locale.clone().into(),
                request.label.clone().into(),
                request.help_text.clone().into(),
                request.facet_label.clone().into(),
                request.seo_label.clone().into(),
            ],
        ))
        .await?;
        for option in &request.options {
            txn.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
INSERT INTO product_attribute_option_translations (id, option_id, locale, label)
VALUES ($1, $2, $3, $4)
ON CONFLICT (option_id, locale) DO UPDATE SET label = EXCLUDED.label
"#,
                vec![
                    generate_id().into(),
                    option.option_id.into(),
                    target_locale.clone().into(),
                    option.label.clone().into(),
                ],
            ))
            .await?;
        }

        let after = build_snapshot(
            load_attribute_aggregate(&txn, tenant_id, attribute_id)
                .await?
                .ok_or(ProductAttributeTranslationError::AttributeNotFound(
                    attribute_id,
                ))?,
            &source_locale,
            &target_locale,
        )?;
        let target = after.target.clone().ok_or_else(|| {
            ProductAttributeTranslationError::OwnerInvariant(
                "exact target locale is absent after Product attribute translation apply".to_string(),
            )
        })?;
        if !record_complete(&target) {
            return Err(ProductAttributeTranslationError::OwnerInvariant(
                "exact target locale is incomplete after Product attribute translation apply"
                    .to_string(),
            ));
        }
        let target_revision = after.target_revision.clone().ok_or_else(|| {
            ProductAttributeTranslationError::OwnerInvariant(
                "exact target revision is absent after Product attribute translation apply"
                    .to_string(),
            )
        })?;

        txn.publish(
            tenant_id,
            actor_user_id,
            DomainEvent::ProductAttributeUpdated { attribute_id },
        )
        .await?;
        let receipt = ExactLocaleApplyReceipt {
            operation_id: current_product_operation_id(),
            attribute_id,
            resource_revision: after.resource_revision,
            target_revision,
            target,
        };
        record_product_operation_result(&receipt)?;
        txn.commit().await?;
        Ok(receipt)
    }

    async fn read_attribute_translation_progress(
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
        let mut after = None;
        let mut facts = ProgressFacts {
            resources: 0,
            required_units: 0,
            exact_required_units: 0,
            optional_units: 0,
            exact_optional_units: 0,
            complete_resources: 0,
        };
        loop {
            let page = load_source_eligible_page(
                &txn,
                tenant_id,
                &source_locale,
                after,
                PROGRESS_PAGE_SIZE,
            )
            .await?;
            if page.is_empty() {
                break;
            }
            for aggregate in &page {
                observe_progress(&mut facts, aggregate, &source_locale, &target_locale)?;
            }
            if page.len() < usize::from(PROGRESS_PAGE_SIZE) {
                break;
            }
            after = page.last().map(|aggregate| aggregate.id);
        }
        txn.commit().await?;
        if facts.exact_required_units > facts.required_units
            || facts.exact_optional_units > facts.optional_units
            || facts.complete_resources > facts.resources
        {
            return Err(ProductAttributeTranslationError::OwnerInvariant(
                "aggregate progress exceeded Product attribute translation bounds".to_string(),
            ));
        }
        Ok(facts)
    }

    async fn attribute_translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> OwnerResult<Option<u64>> {
        validate_uuid(tenant_id, "tenant_id")?;
        ensure_postgres(self.db.get_database_backend())?;
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT MAX(change_seq) AS highwater FROM product_attribute_translation_change_journal WHERE tenant_id = $1",
                vec![tenant_id.into()],
            ))
            .await?
            .ok_or_else(|| {
                ProductAttributeTranslationError::OwnerInvariant(
                    "change high-water query returned no row".to_string(),
                )
            })?;
        optional_positive_sequence(row.try_get("", "highwater")?, "high-water")
    }

    async fn read_attribute_translation_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> OwnerResult<Vec<ChangeRecord>> {
        validate_uuid(tenant_id, "tenant_id")?;
        if through_seq == 0 || after_seq > through_seq {
            return Err(CommerceError::Validation(
                "Product attribute translation change cursor bounds are invalid".to_string(),
            )
            .into());
        }
        if limit == 0 || limit > MAX_CHANGE_PAGE {
            return Err(CommerceError::Validation(format!(
                "Product attribute translation change page size must be between 1 and {MAX_CHANGE_PAGE}"
            ))
            .into());
        }
        ensure_postgres(self.db.get_database_backend())?;
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
SELECT change_seq, attribute_id, resource_revision, lifecycle
FROM product_attribute_translation_change_journal
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

    pub(crate) async fn record_attribute_translation_change_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        attribute_id: Uuid,
        root_event_id: Uuid,
    ) -> CommerceResult<()> {
        if txn.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }
        if tenant_id.is_nil() || attribute_id.is_nil() || root_event_id.is_nil() {
            return Err(CommerceError::Validation(
                "Product attribute translation change identity must not be nil".to_string(),
            ));
        }
        let previous = txn
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
SELECT resource_revision, lifecycle
FROM product_attribute_translation_change_journal
WHERE tenant_id = $1 AND attribute_id = $2
ORDER BY change_seq DESC
LIMIT 1
"#,
                vec![tenant_id.into(), attribute_id.into()],
            ))
            .await?;
        let aggregate = load_attribute_aggregate(txn, tenant_id, attribute_id)
            .await
            .map_err(owner_error_to_commerce)?;
        let (resource_revision, lifecycle) = match aggregate {
            Some(aggregate) => (
                resource_revision(&aggregate),
                if aggregate.archived {
                    ChangeLifecycle::Archived
                } else {
                    ChangeLifecycle::Active
                },
            ),
            None => (
                deleted_revision(root_event_id, attribute_id),
                ChangeLifecycle::Deleted,
            ),
        };
        if let Some(previous) = previous {
            let previous_revision: String = previous.try_get("", "resource_revision")?;
            let previous_lifecycle: String = previous.try_get("", "lifecycle")?;
            if previous_revision == resource_revision && previous_lifecycle == lifecycle.as_str() {
                return Ok(());
            }
        }
        txn.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
INSERT INTO product_attribute_translation_change_journal (
    root_event_id, tenant_id, attribute_id, resource_revision, lifecycle, created_at
) VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP)
ON CONFLICT (root_event_id, attribute_id) DO NOTHING
"#,
            vec![
                root_event_id.into(),
                tenant_id.into(),
                attribute_id.into(),
                resource_revision.into(),
                lifecycle.as_str().into(),
            ],
        ))
        .await?;
        Ok(())
    }
}

#[async_trait]
impl TranslationTargetProvider for ProductAttributeTranslationTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Product owner slug must satisfy Translation contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Product Attribute resource kind must satisfy Translation contract"),
            display_name: "Product attribute copy".to_string(),
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
            .list_attribute_translation_exact_resources(
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
                .attribute_translation_change_highwater(tenant_id)
                .await
                .map_err(owner_error_to_port_error)?;
            let owner = self
                .service
                .read_attribute_translation_progress(
                    tenant_id,
                    request.source_locale.as_str(),
                    request.target_locale.as_str(),
                )
                .await
                .map_err(owner_error_to_port_error)?;
            let after = self
                .service
                .attribute_translation_change_highwater(tenant_id)
                .await
                .map_err(owner_error_to_port_error)?;
            if before != after {
                continue;
            }
            let facts = TranslationTargetProgressFacts {
                required_units: owner.required_units,
                exact_required_units: owner.exact_required_units,
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
                    "product.attribute_translation_progress_invalid",
                    error.to_string(),
                )
            })?;
            return Ok(facts);
        }
        Err(PortError::unavailable(
            "product.attribute_translation_progress_unstable",
            "Product Attribute translation progress changed while it was being aggregated",
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
                    .attribute_translation_change_highwater(tenant_id)
                    .await
                    .map_err(owner_error_to_port_error)?
                    .unwrap_or(after)
                    .max(after);
                (current, after)
            }
            Some(cursor) => cursor,
            None => (
                self.service
                    .attribute_translation_change_highwater(tenant_id)
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
            .read_attribute_translation_changes(tenant_id, after, through, request.limit)
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
                    identity: attribute_identity(change.attribute_id),
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
        let attribute_id = parse_identity(&request.identity)?;
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
            let target = merged_target(&request, &snapshot, &owner)?;
            let applied = with_product_operation_receipt(
                lease,
                self.service.apply_attribute_translation_exact_locale(
                    tenant_id,
                    security.user_id,
                    attribute_id,
                    target,
                ),
            )
            .await
            .map_err(owner_error_to_port_error)?;
            if applied.operation_id != Some(lease.operation_id) {
                return Err(PortError::invariant_violation(
                    "product.attribute_translation_receipt_identity_invalid",
                    "Product Attribute owner receipt is not bound to the active Translation operation",
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
    let attribute_id = parse_identity(&request.identity)?;
    service
        .read_attribute_translation_exact_locale(
            tenant_id,
            attribute_id,
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

async fn load_attribute_aggregate<C>(
    db: &C,
    tenant_id: Uuid,
    attribute_id: Uuid,
) -> OwnerResult<Option<Aggregate>>
where
    C: ConnectionTrait,
{
    AggregateRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!("{} WHERE a.tenant_id = $1 AND a.id = $2", aggregate_select_sql()),
        vec![tenant_id.into(), attribute_id.into()],
    ))
    .one(db)
    .await?
    .map(decode_aggregate)
    .transpose()
}

fn aggregate_select_sql() -> &'static str {
    r#"
SELECT
    a.id,
    a.tenant_id,
    (a.archived_at IS NOT NULL) AS archived,
    a.show_on_storefront,
    COALESCE((
        SELECT jsonb_agg(
            jsonb_build_object(
                'locale', t.locale,
                'label', t.label,
                'help_text', t.help_text,
                'facet_label', t.facet_label,
                'seo_label', t.seo_label
            ) ORDER BY t.locale
        )
        FROM product_attribute_translations t
        WHERE t.attribute_id = a.id
    ), '[]'::jsonb) AS translations,
    COALESCE((
        SELECT jsonb_agg(
            jsonb_build_object(
                'id', o.id,
                'position', o.position,
                'translations', COALESCE((
                    SELECT jsonb_agg(
                        jsonb_build_object('locale', ot.locale, 'label', ot.label)
                        ORDER BY ot.locale
                    )
                    FROM product_attribute_option_translations ot
                    WHERE ot.option_id = o.id
                ), '[]'::jsonb)
            ) ORDER BY o.position, o.id
        )
        FROM product_attribute_options o
        WHERE o.attribute_id = a.id
          AND o.archived_at IS NULL
    ), '[]'::jsonb) AS options
FROM product_attributes a
"#
}

fn source_eligible_sql(with_after: bool) -> String {
    let after = if with_after { "AND a.id > $3" } else { "" };
    let limit = if with_after { "$4" } else { "$3" };
    format!(
        r#"
WITH candidate AS (
    SELECT a.id
    FROM product_attributes a
    INNER JOIN product_attribute_translations source_translation
        ON source_translation.attribute_id = a.id
       AND source_translation.locale = $2
    WHERE a.tenant_id = $1
      AND a.archived_at IS NULL
      {after}
      AND NOT EXISTS (
          SELECT 1
          FROM product_attribute_options source_option
          WHERE source_option.attribute_id = a.id
            AND source_option.archived_at IS NULL
            AND NOT EXISTS (
                SELECT 1
                FROM product_attribute_option_translations source_option_translation
                WHERE source_option_translation.option_id = source_option.id
                  AND source_option_translation.locale = $2
            )
      )
    ORDER BY a.id ASC
    LIMIT {limit}
)
{}
INNER JOIN candidate ON candidate.id = a.id
ORDER BY a.id ASC
"#,
        aggregate_select_sql()
    )
}

fn decode_aggregate(row: AggregateRow) -> OwnerResult<Aggregate> {
    let translations: Vec<StoredTranslation> = serde_json::from_value(row.translations).map_err(|error| {
        ProductAttributeTranslationError::OwnerInvariant(format!(
            "persisted attribute translations are invalid: {error}"
        ))
    })?;
    let options: Vec<StoredOption> = serde_json::from_value(row.options).map_err(|error| {
        ProductAttributeTranslationError::OwnerInvariant(format!(
            "persisted attribute options are invalid: {error}"
        ))
    })?;
    if row.id.is_nil() || row.tenant_id.is_nil() {
        return Err(ProductAttributeTranslationError::OwnerInvariant(
            "persisted Product attribute identity is invalid".to_string(),
        ));
    }
    for translation in &translations {
        validate_stored_translation(translation)?;
    }
    for option in &options {
        if option.id.is_nil() {
            return Err(ProductAttributeTranslationError::OwnerInvariant(
                "persisted Product attribute option identity is invalid".to_string(),
            ));
        }
        for translation in &option.translations {
            validate_stored_option_translation(translation)?;
        }
    }
    Ok(Aggregate {
        id: row.id,
        tenant_id: row.tenant_id,
        archived: row.archived,
        show_on_storefront: row.show_on_storefront,
        translations,
        options,
    })
}

fn build_snapshot(
    aggregate: Aggregate,
    source_locale: &str,
    target_locale: &str,
) -> OwnerResult<ExactLocaleSnapshot> {
    let source = locale_record(&aggregate, source_locale);
    if record_empty(&source) {
        return Err(ProductAttributeTranslationError::SourceLocaleNotFound {
            attribute_id: aggregate.id,
            locale: source_locale.to_string(),
        });
    }
    if !record_complete(&source) {
        return Err(ProductAttributeTranslationError::IncompleteLocale {
            attribute_id: aggregate.id,
            locale: source_locale.to_string(),
        });
    }
    let target_record = locale_record(&aggregate, target_locale);
    let target = (!record_empty(&target_record)).then_some(target_record);
    let resource_revision = resource_revision(&aggregate);
    let source_revision = locale_revision(aggregate.tenant_id, aggregate.id, &source);
    let target_revision = target
        .as_ref()
        .map(|target| locale_revision(aggregate.tenant_id, aggregate.id, target));
    let exact_locales = exact_locales(&aggregate);
    Ok(ExactLocaleSnapshot {
        attribute_id: aggregate.id,
        archived: aggregate.archived,
        show_on_storefront: aggregate.show_on_storefront,
        source_locale: source_locale.to_string(),
        target_locale: target_locale.to_string(),
        resource_revision,
        source_revision,
        target_revision,
        exact_locales,
        source,
        target,
    })
}

fn locale_record(aggregate: &Aggregate, locale: &str) -> LocaleRecord {
    let translation = aggregate
        .translations
        .iter()
        .find(|translation| translation.locale == locale);
    let options = aggregate
        .options
        .iter()
        .map(|option| LocaleOptionRecord {
            option_id: option.id,
            position: option.position,
            label: option
                .translations
                .iter()
                .find(|translation| translation.locale == locale)
                .map(|translation| translation.label.clone()),
        })
        .collect();
    LocaleRecord {
        locale: locale.to_string(),
        label: translation.map(|translation| translation.label.clone()),
        help_text: translation.and_then(|translation| translation.help_text.clone()),
        facet_label: translation.and_then(|translation| translation.facet_label.clone()),
        seo_label: translation.and_then(|translation| translation.seo_label.clone()),
        options,
    }
}

fn record_empty(record: &LocaleRecord) -> bool {
    record.label.is_none()
        && record.help_text.is_none()
        && record.facet_label.is_none()
        && record.seo_label.is_none()
        && record.options.iter().all(|option| option.label.is_none())
}

fn record_complete(record: &LocaleRecord) -> bool {
    record
        .label
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty())
        && record.options.iter().all(|option| {
            option
                .label
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
        })
}

fn exact_locales(aggregate: &Aggregate) -> Vec<String> {
    let mut locales = BTreeSet::new();
    for translation in &aggregate.translations {
        locales.insert(translation.locale.clone());
    }
    for option in &aggregate.options {
        for translation in &option.translations {
            locales.insert(translation.locale.clone());
        }
    }
    locales
        .into_iter()
        .filter(|locale| record_complete(&locale_record(aggregate, locale)))
        .collect()
}

fn resource_revision(aggregate: &Aggregate) -> String {
    let mut hasher = Sha256::new();
    hash_str(&mut hasher, RESOURCE_REVISION_NAMESPACE);
    hash_uuid(&mut hasher, aggregate.tenant_id);
    hash_uuid(&mut hasher, aggregate.id);
    hasher.update([u8::from(aggregate.archived)]);
    hasher.update([u8::from(aggregate.show_on_storefront)]);
    hash_len(&mut hasher, aggregate.translations.len());
    for translation in &aggregate.translations {
        hash_str(&mut hasher, &translation.locale);
        hash_str(&mut hasher, &translation.label);
        hash_optional_str(&mut hasher, translation.help_text.as_deref());
        hash_optional_str(&mut hasher, translation.facet_label.as_deref());
        hash_optional_str(&mut hasher, translation.seo_label.as_deref());
    }
    hash_len(&mut hasher, aggregate.options.len());
    for option in &aggregate.options {
        hash_uuid(&mut hasher, option.id);
        hasher.update(option.position.to_be_bytes());
        hash_len(&mut hasher, option.translations.len());
        for translation in &option.translations {
            hash_str(&mut hasher, &translation.locale);
            hash_str(&mut hasher, &translation.label);
        }
    }
    format!("product-attribute-resource-v1:{}", hex::encode(hasher.finalize()))
}

fn locale_revision(tenant_id: Uuid, attribute_id: Uuid, record: &LocaleRecord) -> String {
    let mut hasher = Sha256::new();
    hash_str(&mut hasher, LOCALE_REVISION_NAMESPACE);
    hash_uuid(&mut hasher, tenant_id);
    hash_uuid(&mut hasher, attribute_id);
    hash_str(&mut hasher, &record.locale);
    hash_optional_str(&mut hasher, record.label.as_deref());
    hash_optional_str(&mut hasher, record.help_text.as_deref());
    hash_optional_str(&mut hasher, record.facet_label.as_deref());
    hash_optional_str(&mut hasher, record.seo_label.as_deref());
    hash_len(&mut hasher, record.options.len());
    for option in &record.options {
        hash_uuid(&mut hasher, option.option_id);
        hasher.update(option.position.to_be_bytes());
        hash_optional_str(&mut hasher, option.label.as_deref());
    }
    format!("product-attribute-locale-v1:{}", hex::encode(hasher.finalize()))
}

fn deleted_revision(root_event_id: Uuid, attribute_id: Uuid) -> String {
    let mut hasher = Sha256::new();
    hash_str(&mut hasher, DELETED_REVISION_NAMESPACE);
    hash_uuid(&mut hasher, root_event_id);
    hash_uuid(&mut hasher, attribute_id);
    format!("attribute-deleted-v1:{}", hex::encode(hasher.finalize()))
}

fn observe_progress(
    facts: &mut ProgressFacts,
    aggregate: &Aggregate,
    source_locale: &str,
    target_locale: &str,
) -> OwnerResult<()> {
    let source = locale_record(aggregate, source_locale);
    if !record_complete(&source) {
        return Err(ProductAttributeTranslationError::IncompleteLocale {
            attribute_id: aggregate.id,
            locale: source_locale.to_string(),
        });
    }
    let target = locale_record(aggregate, target_locale);
    checked_add(&mut facts.resources, 1, "resources")?;
    let required_units = 1_u64
        .checked_add(source.options.len() as u64)
        .ok_or_else(|| overflow("required_units"))?;
    checked_add(&mut facts.required_units, required_units, "required_units")?;
    let exact_required = u64::from(
        target
            .label
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
    ) + target
        .options
        .iter()
        .filter(|option| {
            option
                .label
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
        })
        .count() as u64;
    checked_add(
        &mut facts.exact_required_units,
        exact_required,
        "exact_required_units",
    )?;
    for (source_value, target_value) in [
        (source.help_text.as_ref(), target.help_text.as_ref()),
        (source.facet_label.as_ref(), target.facet_label.as_ref()),
        (source.seo_label.as_ref(), target.seo_label.as_ref()),
    ] {
        if source_value.is_some() {
            checked_add(&mut facts.optional_units, 1, "optional_units")?;
            if target_value.is_some_and(|value| !value.trim().is_empty()) {
                checked_add(
                    &mut facts.exact_optional_units,
                    1,
                    "exact_optional_units",
                )?;
            }
        }
    }
    if exact_required == required_units {
        checked_add(&mut facts.complete_resources, 1, "complete_resources")?;
    }
    Ok(())
}

fn summary_from_owner(owner: &ExactLocaleSnapshot) -> Result<TranslationResourceSummary, PortError> {
    Ok(TranslationResourceSummary {
        identity: attribute_identity(owner.attribute_id),
        display_label: owner.source.label.clone().ok_or_else(|| {
            PortError::invariant_violation(
                "product.attribute_translation_source_invalid",
                "Product Attribute source label is missing",
            )
        })?,
        lifecycle: if owner.archived {
            TranslationResourceLifecycle::Archived
        } else {
            TranslationResourceLifecycle::Active
        },
        resource_revision: opaque_revision(owner.resource_revision.clone(), "resource_revision")?,
        exact_locales: owner
            .exact_locales
            .iter()
            .map(|locale| {
                TenantLocale::new(locale.clone()).map_err(|error| {
                    PortError::invariant_violation(
                        "product.attribute_translation_locale_invalid",
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
            "product.attribute_translation_locale_identity_invalid",
            "Product Attribute owner returned different exact locales",
        ));
    }
    let classification = if owner.show_on_storefront {
        TranslationDataClassification::Public
    } else {
        TranslationDataClassification::TenantPrivate
    };
    let ai_export_allowed = owner.show_on_storefront;
    let target = owner.target.as_ref();
    let mut fields = Vec::with_capacity(4 + owner.source.options.len());
    fields.push(text_field(
        ATTRIBUTE_LABEL_FIELD,
        owner.source.label.as_deref().ok_or_else(|| {
            PortError::invariant_violation(
                "product.attribute_translation_source_invalid",
                "Product Attribute source label is missing",
            )
        })?,
        target.and_then(|target| target.label.clone()),
        TranslationValueProfile::PlainText,
        true,
        Some(255),
        classification,
        ai_export_allowed,
    )?);
    if let Some(source) = owner.source.help_text.as_deref() {
        fields.push(text_field(
            ATTRIBUTE_HELP_TEXT_FIELD,
            source,
            target.and_then(|target| target.help_text.clone()),
            TranslationValueProfile::PlainText,
            false,
            None,
            classification,
            ai_export_allowed,
        )?);
    }
    if let Some(source) = owner.source.facet_label.as_deref() {
        fields.push(text_field(
            ATTRIBUTE_FACET_LABEL_FIELD,
            source,
            target.and_then(|target| target.facet_label.clone()),
            TranslationValueProfile::PlainText,
            false,
            Some(255),
            classification,
            ai_export_allowed,
        )?);
    }
    if let Some(source) = owner.source.seo_label.as_deref() {
        fields.push(text_field(
            ATTRIBUTE_SEO_LABEL_FIELD,
            source,
            target.and_then(|target| target.seo_label.clone()),
            TranslationValueProfile::SeoText,
            false,
            Some(255),
            classification,
            ai_export_allowed,
        )?);
    }
    for source_option in &owner.source.options {
        let source = source_option.label.as_deref().ok_or_else(|| {
            PortError::invariant_violation(
                "product.attribute_translation_source_invalid",
                "Product Attribute source option label is missing",
            )
        })?;
        let target_value = target
            .and_then(|target| {
                target
                    .options
                    .iter()
                    .find(|option| option.option_id == source_option.option_id)
            })
            .and_then(|option| option.label.clone());
        fields.push(text_field(
            &format!("{OPTION_LABEL_PREFIX}{}", source_option.option_id),
            source,
            target_value,
            TranslationValueProfile::PlainText,
            true,
            Some(255),
            classification,
            ai_export_allowed,
        )?);
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
        fields,
    };
    snapshot.validate().map_err(|error| {
        PortError::invariant_violation(
            "product.attribute_translation_snapshot_invalid",
            error.to_string(),
        )
    })?;
    Ok(snapshot)
}

#[allow(clippy::too_many_arguments)]
fn text_field(
    key: &str,
    source_value: &str,
    exact_target_value: Option<String>,
    profile: TranslationValueProfile,
    required: bool,
    max_characters: Option<u32>,
    classification: TranslationDataClassification,
    ai_export_allowed: bool,
) -> Result<TranslationFieldSnapshot, PortError> {
    Ok(TranslationFieldSnapshot {
        descriptor: TranslationFieldDescriptor {
            key: FieldKey::new(key).map_err(|error| {
                PortError::invariant_violation(
                    "product.attribute_translation_field_key_invalid",
                    error.to_string(),
                )
            })?,
            profile,
            strategy: TranslationStrategy::Translate,
            classification,
            required,
            ai_export_allowed,
            max_characters,
            preserves_whitespace: false,
        },
        source_value: source_value.to_string(),
        exact_target_value,
        source_hash: field_hash(source_value),
        protected_tokens: Vec::new(),
    })
}

fn merged_target(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
    owner: &ExactLocaleSnapshot,
) -> Result<ExactLocaleApply, PortError> {
    let mut values = merged_patch_values(request, snapshot);
    let label = required_target_value(
        values.remove(ATTRIBUTE_LABEL_FIELD).flatten(),
        "attribute label",
    )?;
    let target = owner.target.as_ref();
    let help_text = optional_merged_value(
        &mut values,
        ATTRIBUTE_HELP_TEXT_FIELD,
        owner.source.help_text.is_some(),
        target.and_then(|target| target.help_text.clone()),
    );
    let facet_label = optional_merged_value(
        &mut values,
        ATTRIBUTE_FACET_LABEL_FIELD,
        owner.source.facet_label.is_some(),
        target.and_then(|target| target.facet_label.clone()),
    );
    let seo_label = optional_merged_value(
        &mut values,
        ATTRIBUTE_SEO_LABEL_FIELD,
        owner.source.seo_label.is_some(),
        target.and_then(|target| target.seo_label.clone()),
    );
    let mut options = Vec::with_capacity(owner.source.options.len());
    for source_option in &owner.source.options {
        let key = format!("{OPTION_LABEL_PREFIX}{}", source_option.option_id);
        let label = required_target_value(
            values.remove(key.as_str()).flatten(),
            "attribute option label",
        )?;
        options.push(OptionApply {
            option_id: source_option.option_id,
            label,
        });
    }
    Ok(ExactLocaleApply {
        source_locale: request.source_locale.as_str().to_string(),
        target_locale: request.target_locale.as_str().to_string(),
        label,
        help_text,
        facet_label,
        seo_label,
        options,
        expected_resource_revision: request.expected_resource_revision.as_str().to_string(),
        expected_source_revision: request.expected_source_revision.as_str().to_string(),
        expected_target_revision: request
            .expected_target_revision
            .as_ref()
            .map(|revision| revision.as_str().to_string()),
    })
}

fn optional_merged_value(
    values: &mut std::collections::BTreeMap<String, Option<String>>,
    key: &str,
    source_visible: bool,
    target_only: Option<String>,
) -> Option<String> {
    if source_visible {
        values
            .remove(key)
            .flatten()
            .and_then(normalize_optional_target_value)
    } else {
        target_only
    }
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
            "product.attribute_translation_receipt_identity_missing",
            "Product Attribute translation owner receipt is missing operation identity",
        )
    })?;
    Ok(TranslationApplicationReceipt {
        provider_receipt_id: format!("product-attribute:{operation_id}"),
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

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "product.invalid_tenant_id",
            "Product Attribute translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Products, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "product.attribute_translation_permission_denied",
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
            "product.attribute_translation_identity_invalid",
            "Product Attribute translation identity must address product/attribute without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "product.attribute_translation_resource_id_invalid",
            "Product Attribute translation resource id must be a UUID",
        )
    })
}

fn attribute_identity(attribute_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Product owner slug must satisfy Translation contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Product Attribute resource kind must satisfy Translation contract"),
        resource_id: ResourceId::new(attribute_id.to_string())
            .expect("Product Attribute UUID must satisfy Translation resource id contract"),
        subresource_id: None,
    }
}

fn resource_cursor(attribute_id: Uuid) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(attribute_id.to_string()).map_err(|error| {
        PortError::invariant_violation(
            "product.attribute_translation_resource_cursor_invalid",
            error.to_string(),
        )
    })
}

fn parse_resource_cursor(value: &str) -> Result<Uuid, PortError> {
    Uuid::parse_str(value).map_err(|_| {
        PortError::validation(
            "product.attribute_translation_resource_cursor_invalid",
            "Product Attribute translation resource cursor must be a UUID",
        )
    })
}

fn change_cursor(through: u64, after: u64) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(format!("{CHANGE_CURSOR_VERSION}:{through}:{after}")).map_err(|error| {
        PortError::invariant_violation(
            "product.attribute_translation_change_cursor_invalid",
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
            "product.attribute_translation_change_cursor_invalid",
            "Product Attribute translation change cursor is invalid",
        ));
    }
    let through = through.unwrap_or_default();
    let after = after.unwrap_or_default();
    if through == 0 || after == 0 || after > through {
        return Err(PortError::validation(
            "product.attribute_translation_change_cursor_invalid",
            "Product Attribute translation change cursor bounds are invalid",
        ));
    }
    Ok((through, after))
}

fn opaque_revision(value: String, field: &'static str) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(value).map_err(|error| {
        PortError::invariant_violation(
            "product.attribute_translation_revision_invalid",
            format!("Product Attribute {field} is invalid: {error}"),
        )
    })
}

fn owner_error_to_port_error(error: ProductAttributeTranslationError) -> PortError {
    match error {
        ProductAttributeTranslationError::AttributeNotFound(_) => PortError::not_found(
            "product.attribute_translation_resource_not_found",
            "Product Attribute translation resource was not found",
        ),
        ProductAttributeTranslationError::SourceLocaleNotFound { .. } => PortError::not_found(
            "product.attribute_translation_source_not_found",
            "Exact source Product Attribute locale was not found",
        ),
        ProductAttributeTranslationError::IncompleteLocale { .. }
        | ProductAttributeTranslationError::OwnerInvariant(_) => PortError::invariant_violation(
            "product.attribute_translation_owner_invariant",
            "Product Attribute translation owner state is invalid",
        ),
        ProductAttributeTranslationError::RevisionConflict { .. }
        | ProductAttributeTranslationError::OptionSetMismatch => PortError::conflict(
            "product.attribute_translation_revision_conflict",
            "Product Attribute translation state conflicts with the requested mutation",
        ),
        ProductAttributeTranslationError::Commerce(error) => product_error_to_port_error(error),
    }
}

fn product_error_to_port_error(error: CommerceError) -> PortError {
    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "product.attribute_translation_owner_unavailable",
            "Product Attribute translation storage is temporarily unavailable",
        ),
        CommerceError::ProductNotFound(_) => PortError::not_found(
            "product.attribute_translation_resource_not_found",
            "Product Attribute translation resource was not found",
        ),
        CommerceError::DuplicateHandle { .. } | CommerceError::DuplicateSku(_) => {
            PortError::conflict(
                "product.attribute_translation_owner_conflict",
                "Product state conflicts with the requested Attribute translation mutation",
            )
        }
        CommerceError::Validation(_) | CommerceError::NoVariants => PortError::validation(
            "product.attribute_translation_owner_validation",
            "Product rejected the Attribute translation mutation",
        ),
        CommerceError::CannotDeletePublished => PortError::conflict(
            "product.attribute_translation_owner_conflict",
            "Product state conflicts with the requested Attribute translation mutation",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "product.attribute_translation_owner_invariant",
            "Product Attribute translation state is invalid",
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
            "Product attribute translation source and target locale must differ".to_string(),
        )
        .into());
    }
    Ok(())
}

fn validate_stored_translation(translation: &StoredTranslation) -> OwnerResult<()> {
    if canonical_locale(&translation.locale)? != translation.locale
        || translation.label.trim().is_empty()
        || translation.label.chars().count() > 255
    {
        return Err(ProductAttributeTranslationError::OwnerInvariant(
            "persisted Product attribute translation violates its owner contract".to_string(),
        ));
    }
    validate_optional_persisted(translation.help_text.as_deref(), None)?;
    validate_optional_persisted(translation.facet_label.as_deref(), Some(255))?;
    validate_optional_persisted(translation.seo_label.as_deref(), Some(255))?;
    Ok(())
}

fn validate_stored_option_translation(translation: &StoredOptionTranslation) -> OwnerResult<()> {
    if canonical_locale(&translation.locale)? != translation.locale
        || translation.label.trim().is_empty()
        || translation.label.chars().count() > 255
    {
        return Err(ProductAttributeTranslationError::OwnerInvariant(
            "persisted Product attribute option translation violates its owner contract".to_string(),
        ));
    }
    Ok(())
}

fn validate_optional_persisted(value: Option<&str>, max: Option<usize>) -> OwnerResult<()> {
    if let Some(value) = value {
        if value.trim().is_empty() || max.is_some_and(|max| value.chars().count() > max) {
            return Err(ProductAttributeTranslationError::OwnerInvariant(
                "persisted optional Product attribute copy is blank or too long".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_target_value(value: &str, label: &str, max: Option<usize>) -> OwnerResult<()> {
    if value.trim().is_empty()
        || value.trim() != value
        || max.is_some_and(|max| value.chars().count() > max)
    {
        return Err(CommerceError::Validation(format!(
            "Product attribute translation {label} must be normalized, nonblank, and within its length limit"
        ))
        .into());
    }
    Ok(())
}

fn validate_optional_target_value(
    value: Option<&str>,
    label: &str,
    max: Option<usize>,
) -> OwnerResult<()> {
    if let Some(value) = value {
        validate_target_value(value, label, max)?;
    }
    Ok(())
}

fn validate_uuid(value: Uuid, field: &str) -> OwnerResult<()> {
    if value.is_nil() {
        return Err(CommerceError::Validation(format!(
            "Product attribute translation {field} must not be nil"
        ))
        .into());
    }
    Ok(())
}

fn ensure_revision(revision: &'static str, expected: &str, current: &str) -> OwnerResult<()> {
    if expected != current {
        return Err(ProductAttributeTranslationError::RevisionConflict { revision });
    }
    Ok(())
}

fn ensure_postgres(backend: DatabaseBackend) -> OwnerResult<()> {
    if backend != DatabaseBackend::Postgres {
        return Err(CommerceError::Validation(
            "Product attribute Translation target requires PostgreSQL".to_string(),
        )
        .into());
    }
    Ok(())
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

fn invalid_sequence(field: &str) -> ProductAttributeTranslationError {
    ProductAttributeTranslationError::OwnerInvariant(format!(
        "Product attribute translation change {field} sequence must be positive"
    ))
}

fn change_record_from_row(row: sea_orm::QueryResult) -> OwnerResult<ChangeRecord> {
    let change_seq = positive_sequence(row.try_get("", "change_seq")?, "change")?;
    let attribute_id: Uuid = row.try_get("", "attribute_id")?;
    let resource_revision: String = row.try_get("", "resource_revision")?;
    let lifecycle: String = row.try_get("", "lifecycle")?;
    if attribute_id.is_nil() || resource_revision.trim().is_empty() {
        return Err(ProductAttributeTranslationError::OwnerInvariant(
            "Product attribute translation change row is invalid".to_string(),
        ));
    }
    Ok(ChangeRecord {
        change_seq,
        attribute_id,
        resource_revision,
        lifecycle: ChangeLifecycle::parse(&lifecycle)?,
    })
}

fn owner_error_to_commerce(error: ProductAttributeTranslationError) -> CommerceError {
    match error {
        ProductAttributeTranslationError::Commerce(error) => error,
        error => CommerceError::Validation(error.to_string()),
    }
}

fn checked_add(value: &mut u64, increment: u64, label: &str) -> OwnerResult<()> {
    *value = value.checked_add(increment).ok_or_else(|| overflow(label))?;
    Ok(())
}

fn overflow(label: &str) -> ProductAttributeTranslationError {
    ProductAttributeTranslationError::OwnerInvariant(format!(
        "Product attribute translation progress `{label}` overflowed u64"
    ))
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
