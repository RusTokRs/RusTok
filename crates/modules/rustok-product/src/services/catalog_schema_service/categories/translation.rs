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
const TRANSLATION_RESOURCE_KIND: &str = "category_form";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_category_form_patch";
const GROUP_LABEL_PREFIX: &str = "group:";
const CHANGE_CURSOR_VERSION: &str = "v1";
const MAX_CHANGE_PAGE: u16 = 200;
const PROGRESS_PAGE_SIZE: u16 = 200;
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;
const RESOURCE_REVISION_NAMESPACE: &str = "rustok-product/category-form-translation-resource/v1";
const LOCALE_REVISION_NAMESPACE: &str = "rustok-product/category-form-translation-locale/v1";
const DELETED_REVISION_NAMESPACE: &str = "rustok-product/category-form-translation-deleted/v1";

#[derive(Debug, Error)]
enum ProductCategoryFormTranslationError {
    #[error(transparent)]
    Commerce(#[from] CommerceError),
    #[error("Product category form translation resource not found: {0}")]
    CategoryFormNotFound(Uuid),
    #[error("Product category form translation source locale not found: {locale} for category {category_id}")]
    SourceLocaleNotFound { category_id: Uuid, locale: String },
    #[error("Product category form translation locale is incomplete: {locale} for category {category_id}")]
    IncompleteLocale { category_id: Uuid, locale: String },
    #[error("Product category form translation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },
    #[error("Product category form translation group set changed while the mutation was prepared")]
    GroupSetMismatch,
    #[error("Product category form translation owner invariant failed: {0}")]
    OwnerInvariant(String),
}

impl From<sea_orm::DbErr> for ProductCategoryFormTranslationError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Commerce(CommerceError::Database(error))
    }
}

type OwnerResult<T> = Result<T, ProductCategoryFormTranslationError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct LocaleGroupRecord {
    group_id: Uuid,
    position: i32,
    label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct LocaleRecord {
    locale: String,
    groups: Vec<LocaleGroupRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExactLocaleSnapshot {
    category_id: Uuid,
    category_code: String,
    active: bool,
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
struct GroupApply {
    group_id: Uuid,
    label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExactLocaleApply {
    source_locale: String,
    target_locale: String,
    groups: Vec<GroupApply>,
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
    target: LocaleRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProgressFacts {
    resources: u64,
    required_units: u64,
    exact_required_units: u64,
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
            _ => Err(ProductCategoryFormTranslationError::OwnerInvariant(
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
struct AggregateRow {
    id: Uuid,
    tenant_id: Uuid,
    code: String,
    active: bool,
    groups: JsonValue,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredGroupTranslation {
    locale: String,
    label: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredGroup {
    id: Uuid,
    position: i32,
    translations: Vec<StoredGroupTranslation>,
}

#[derive(Debug, Clone)]
struct Aggregate {
    id: Uuid,
    tenant_id: Uuid,
    code: String,
    active: bool,
    groups: Vec<StoredGroup>,
}

#[derive(Clone)]
struct ProductCategoryFormTranslationTargetProvider {
    service: Arc<ProductCatalogSchemaService>,
}

impl ProductCatalogSchemaService {
    /// Builds the neutral Translation adapter for Product-owned category form presentation.
    ///
    /// Category name/slug/description remain Taxonomy-owned. This resource contains only local
    /// Product form-group labels attached to a structural category.
    pub fn category_form_translation_target_provider(
        service: Arc<Self>,
    ) -> impl TranslationTargetProvider {
        ProductCategoryFormTranslationTargetProvider { service }
    }

    async fn list_category_form_translation_exact_resources(
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
                "Product category form translation resource page size must be between 1 and 200"
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

    async fn read_category_form_translation_exact_locale(
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
        let aggregate = load_category_form_aggregate(&self.db, tenant_id, category_id)
            .await?
            .filter(|aggregate| !aggregate.groups.is_empty())
            .ok_or(ProductCategoryFormTranslationError::CategoryFormNotFound(
                category_id,
            ))?;
        build_snapshot(aggregate, &source_locale, &target_locale)
    }

    async fn apply_category_form_translation_exact_locale(
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
        let mut requested_group_ids = BTreeSet::new();
        for group in &request.groups {
            validate_uuid(group.group_id, "group_id")?;
            validate_target_value(&group.label, "group label", Some(255))?;
            if !requested_group_ids.insert(group.group_id) {
                return Err(ProductCategoryFormTranslationError::GroupSetMismatch);
            }
        }
        ensure_postgres(self.db.get_database_backend())?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        let locked = txn
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
SELECT id
FROM catalog_categories
WHERE tenant_id = $1 AND id = $2 AND kind = 'structural' AND deleted_at IS NULL
FOR UPDATE
"#,
                vec![tenant_id.into(), category_id.into()],
            ))
            .await?;
        if locked.is_none() {
            return Err(ProductCategoryFormTranslationError::CategoryFormNotFound(
                category_id,
            ));
        }
        txn.query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT id FROM category_attribute_groups WHERE tenant_id = $1 AND category_id = $2 ORDER BY id FOR UPDATE",
            vec![tenant_id.into(), category_id.into()],
        ))
        .await?;

        let before = build_snapshot(
            load_category_form_aggregate(&txn, tenant_id, category_id)
                .await?
                .filter(|aggregate| !aggregate.groups.is_empty())
                .ok_or(ProductCategoryFormTranslationError::CategoryFormNotFound(
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
            return Err(ProductCategoryFormTranslationError::RevisionConflict {
                revision: "target",
            });
        }
        let current_group_ids = before
            .source
            .groups
            .iter()
            .map(|group| group.group_id)
            .collect::<BTreeSet<_>>();
        if current_group_ids != requested_group_ids {
            return Err(ProductCategoryFormTranslationError::GroupSetMismatch);
        }

        for group in &request.groups {
            txn.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
INSERT INTO category_attribute_group_translations (id, group_id, locale, label)
VALUES ($1, $2, $3, $4)
ON CONFLICT (group_id, locale) DO UPDATE SET label = EXCLUDED.label
"#,
                vec![
                    generate_id().into(),
                    group.group_id.into(),
                    target_locale.clone().into(),
                    group.label.clone().into(),
                ],
            ))
            .await?;
        }

        let after = build_snapshot(
            load_category_form_aggregate(&txn, tenant_id, category_id)
                .await?
                .filter(|aggregate| !aggregate.groups.is_empty())
                .ok_or(ProductCategoryFormTranslationError::CategoryFormNotFound(
                    category_id,
                ))?,
            &source_locale,
            &target_locale,
        )?;
        let target = after.target.clone().ok_or_else(|| {
            ProductCategoryFormTranslationError::OwnerInvariant(
                "exact target locale is absent after Product category form translation apply"
                    .to_string(),
            )
        })?;
        if !record_complete(&target) {
            return Err(ProductCategoryFormTranslationError::OwnerInvariant(
                "exact target locale is incomplete after Product category form translation apply"
                    .to_string(),
            ));
        }
        let target_revision = after.target_revision.clone().ok_or_else(|| {
            ProductCategoryFormTranslationError::OwnerInvariant(
                "exact target revision is absent after Product category form translation apply"
                    .to_string(),
            )
        })?;

        txn.publish(
            tenant_id,
            actor_user_id,
            DomainEvent::CatalogCategoryAttributesChanged { category_id },
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

    async fn read_category_form_translation_progress(
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
            || facts.complete_resources > facts.resources
        {
            return Err(ProductCategoryFormTranslationError::OwnerInvariant(
                "aggregate progress exceeded Product category form translation bounds".to_string(),
            ));
        }
        Ok(facts)
    }

    async fn category_form_translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> OwnerResult<Option<u64>> {
        validate_uuid(tenant_id, "tenant_id")?;
        ensure_postgres(self.db.get_database_backend())?;
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT MAX(change_seq) AS highwater FROM product_category_form_translation_change_journal WHERE tenant_id = $1",
                vec![tenant_id.into()],
            ))
            .await?
            .ok_or_else(|| {
                ProductCategoryFormTranslationError::OwnerInvariant(
                    "change high-water query returned no row".to_string(),
                )
            })?;
        optional_positive_sequence(row.try_get("", "highwater")?, "high-water")
    }

    async fn read_category_form_translation_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> OwnerResult<Vec<ChangeRecord>> {
        validate_uuid(tenant_id, "tenant_id")?;
        if through_seq == 0 || after_seq > through_seq {
            return Err(CommerceError::Validation(
                "Product category form translation change cursor bounds are invalid".to_string(),
            )
            .into());
        }
        if limit == 0 || limit > MAX_CHANGE_PAGE {
            return Err(CommerceError::Validation(format!(
                "Product category form translation change page size must be between 1 and {MAX_CHANGE_PAGE}"
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
FROM product_category_form_translation_change_journal
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

    pub(crate) async fn record_category_form_translation_change_in_tx(
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
                "Product category form translation change identity must not be nil".to_string(),
            ));
        }
        let previous = txn
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
SELECT resource_revision, lifecycle
FROM product_category_form_translation_change_journal
WHERE tenant_id = $1 AND category_id = $2
ORDER BY change_seq DESC
LIMIT 1
"#,
                vec![tenant_id.into(), category_id.into()],
            ))
            .await?;
        let aggregate = load_category_form_aggregate(txn, tenant_id, category_id)
            .await
            .map_err(owner_error_to_commerce)?;
        let live = aggregate.filter(|aggregate| !aggregate.groups.is_empty());
        let (resource_revision, lifecycle) = match live {
            Some(aggregate) => (
                resource_revision(&aggregate),
                if aggregate.active {
                    ChangeLifecycle::Active
                } else {
                    ChangeLifecycle::Archived
                },
            ),
            None if previous.is_none() => return Ok(()),
            None => (
                deleted_revision(root_event_id, category_id),
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
INSERT INTO product_category_form_translation_change_journal (
    root_event_id, tenant_id, category_id, resource_revision, lifecycle, created_at
) VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP)
ON CONFLICT (root_event_id, category_id) DO NOTHING
"#,
            vec![
                root_event_id.into(),
                tenant_id.into(),
                category_id.into(),
                resource_revision.into(),
                lifecycle.as_str().into(),
            ],
        ))
        .await?;
        Ok(())
    }
}

#[async_trait]
impl TranslationTargetProvider for ProductCategoryFormTranslationTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
                .expect("static Product owner slug must satisfy Translation contract"),
            resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
                .expect("static Product Category Form resource kind must satisfy Translation contract"),
            display_name: "Product category form copy".to_string(),
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
            .list_category_form_translation_exact_resources(
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
                .category_form_translation_change_highwater(tenant_id)
                .await
                .map_err(owner_error_to_port_error)?;
            let owner = self
                .service
                .read_category_form_translation_progress(
                    tenant_id,
                    request.source_locale.as_str(),
                    request.target_locale.as_str(),
                )
                .await
                .map_err(owner_error_to_port_error)?;
            let after = self
                .service
                .category_form_translation_change_highwater(tenant_id)
                .await
                .map_err(owner_error_to_port_error)?;
            if before != after {
                continue;
            }
            let facts = TranslationTargetProgressFacts {
                required_units: owner.required_units,
                exact_required_units: owner.exact_required_units,
                optional_units: 0,
                exact_optional_units: 0,
                resources: owner.resources,
                complete_resources: owner.complete_resources,
                owner_change_cursor: after
                    .map(|change_seq| change_cursor(change_seq, change_seq))
                    .transpose()?,
            };
            facts.validate().map_err(|error| {
                PortError::invariant_violation(
                    "product.category_form_translation_progress_invalid",
                    error.to_string(),
                )
            })?;
            return Ok(facts);
        }
        Err(PortError::unavailable(
            "product.category_form_translation_progress_unstable",
            "Product Category Form translation progress changed while it was being aggregated",
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
                    .category_form_translation_change_highwater(tenant_id)
                    .await
                    .map_err(owner_error_to_port_error)?
                    .unwrap_or(after)
                    .max(after);
                (current, after)
            }
            Some(cursor) => cursor,
            None => (
                self.service
                    .category_form_translation_change_highwater(tenant_id)
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
            .read_category_form_translation_changes(tenant_id, after, through, request.limit)
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
                    identity: category_form_identity(change.category_id),
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
            let target = merged_target(&request, &snapshot, &owner)?;
            let applied = with_product_operation_receipt(
                lease,
                self.service.apply_category_form_translation_exact_locale(
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
                    "product.category_form_translation_receipt_identity_invalid",
                    "Product Category Form owner receipt is not bound to the active Translation operation",
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
        .read_category_form_translation_exact_locale(
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

async fn load_category_form_aggregate<C>(
    db: &C,
    tenant_id: Uuid,
    category_id: Uuid,
) -> OwnerResult<Option<Aggregate>>
where
    C: ConnectionTrait,
{
    let row = AggregateRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!(
            "{} WHERE c.tenant_id = $1 AND c.id = $2 AND c.kind = 'structural' AND c.deleted_at IS NULL",
            aggregate_select_sql()
        ),
        vec![tenant_id.into(), category_id.into()],
    ))
    .one(db)
    .await?;
    row.map(decode_aggregate).transpose()
}

fn aggregate_select_sql() -> &'static str {
    r#"
SELECT
    c.id,
    c.tenant_id,
    c.code,
    c.is_active AS active,
    COALESCE((
        SELECT jsonb_agg(
            jsonb_build_object(
                'id', g.id,
                'position', g.position,
                'translations', COALESCE((
                    SELECT jsonb_agg(
                        jsonb_build_object('locale', gt.locale, 'label', gt.label)
                        ORDER BY gt.locale
                    )
                    FROM category_attribute_group_translations gt
                    WHERE gt.group_id = g.id
                ), '[]'::jsonb)
            ) ORDER BY g.position, g.id
        )
        FROM category_attribute_groups g
        WHERE g.tenant_id = c.tenant_id AND g.category_id = c.id
    ), '[]'::jsonb) AS groups
FROM catalog_categories c
"#
}

fn source_eligible_sql(with_after: bool) -> String {
    let after = if with_after { "AND c.id > $3" } else { "" };
    let limit = if with_after { "$4" } else { "$3" };
    format!(
        r#"
WITH candidate AS (
    SELECT c.id
    FROM catalog_categories c
    WHERE c.tenant_id = $1
      AND c.kind = 'structural'
      AND c.deleted_at IS NULL
      AND c.is_active = TRUE
      {after}
      AND EXISTS (
          SELECT 1
          FROM category_attribute_groups source_group
          WHERE source_group.tenant_id = c.tenant_id
            AND source_group.category_id = c.id
      )
      AND NOT EXISTS (
          SELECT 1
          FROM category_attribute_groups source_group
          WHERE source_group.tenant_id = c.tenant_id
            AND source_group.category_id = c.id
            AND NOT EXISTS (
                SELECT 1
                FROM category_attribute_group_translations source_group_translation
                WHERE source_group_translation.group_id = source_group.id
                  AND source_group_translation.locale = $2
            )
      )
    ORDER BY c.id ASC
    LIMIT {limit}
)
{}
INNER JOIN candidate ON candidate.id = c.id
ORDER BY c.id ASC
"#,
        aggregate_select_sql()
    )
}

fn decode_aggregate(row: AggregateRow) -> OwnerResult<Aggregate> {
    let groups: Vec<StoredGroup> = serde_json::from_value(row.groups).map_err(|error| {
        ProductCategoryFormTranslationError::OwnerInvariant(format!(
            "persisted Product category form groups are invalid: {error}"
        ))
    })?;
    if row.id.is_nil() || row.tenant_id.is_nil() || row.code.trim().is_empty() {
        return Err(ProductCategoryFormTranslationError::OwnerInvariant(
            "persisted Product category form identity is invalid".to_string(),
        ));
    }
    for group in &groups {
        if group.id.is_nil() {
            return Err(ProductCategoryFormTranslationError::OwnerInvariant(
                "persisted Product category form group identity is invalid".to_string(),
            ));
        }
        for translation in &group.translations {
            validate_stored_group_translation(translation)?;
        }
    }
    Ok(Aggregate {
        id: row.id,
        tenant_id: row.tenant_id,
        code: row.code,
        active: row.active,
        groups,
    })
}

fn build_snapshot(
    aggregate: Aggregate,
    source_locale: &str,
    target_locale: &str,
) -> OwnerResult<ExactLocaleSnapshot> {
    if aggregate.groups.is_empty() {
        return Err(ProductCategoryFormTranslationError::CategoryFormNotFound(
            aggregate.id,
        ));
    }
    let source = locale_record(&aggregate, source_locale);
    if record_empty(&source) {
        return Err(ProductCategoryFormTranslationError::SourceLocaleNotFound {
            category_id: aggregate.id,
            locale: source_locale.to_string(),
        });
    }
    if !record_complete(&source) {
        return Err(ProductCategoryFormTranslationError::IncompleteLocale {
            category_id: aggregate.id,
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
        category_id: aggregate.id,
        category_code: aggregate.code,
        active: aggregate.active,
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
    LocaleRecord {
        locale: locale.to_string(),
        groups: aggregate
            .groups
            .iter()
            .map(|group| LocaleGroupRecord {
                group_id: group.id,
                position: group.position,
                label: group
                    .translations
                    .iter()
                    .find(|translation| translation.locale == locale)
                    .map(|translation| translation.label.clone()),
            })
            .collect(),
    }
}

fn record_empty(record: &LocaleRecord) -> bool {
    record.groups.iter().all(|group| group.label.is_none())
}

fn record_complete(record: &LocaleRecord) -> bool {
    !record.groups.is_empty()
        && record.groups.iter().all(|group| {
            group
                .label
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
        })
}

fn exact_locales(aggregate: &Aggregate) -> Vec<String> {
    let mut locales = BTreeSet::new();
    for group in &aggregate.groups {
        for translation in &group.translations {
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
    hasher.update([u8::from(aggregate.active)]);
    hash_len(&mut hasher, aggregate.groups.len());
    for group in &aggregate.groups {
        hash_uuid(&mut hasher, group.id);
        hasher.update(group.position.to_be_bytes());
        hash_len(&mut hasher, group.translations.len());
        for translation in &group.translations {
            hash_str(&mut hasher, &translation.locale);
            hash_str(&mut hasher, &translation.label);
        }
    }
    format!(
        "product-category-form-resource-v1:{}",
        hex::encode(hasher.finalize())
    )
}

fn locale_revision(tenant_id: Uuid, category_id: Uuid, record: &LocaleRecord) -> String {
    let mut hasher = Sha256::new();
    hash_str(&mut hasher, LOCALE_REVISION_NAMESPACE);
    hash_uuid(&mut hasher, tenant_id);
    hash_uuid(&mut hasher, category_id);
    hash_str(&mut hasher, &record.locale);
    hash_len(&mut hasher, record.groups.len());
    for group in &record.groups {
        hash_uuid(&mut hasher, group.group_id);
        hasher.update(group.position.to_be_bytes());
        hash_optional_str(&mut hasher, group.label.as_deref());
    }
    format!(
        "product-category-form-locale-v1:{}",
        hex::encode(hasher.finalize())
    )
}

fn deleted_revision(root_event_id: Uuid, category_id: Uuid) -> String {
    let mut hasher = Sha256::new();
    hash_str(&mut hasher, DELETED_REVISION_NAMESPACE);
    hash_uuid(&mut hasher, root_event_id);
    hash_uuid(&mut hasher, category_id);
    format!(
        "category-form-deleted-v1:{}",
        hex::encode(hasher.finalize())
    )
}

fn observe_progress(
    facts: &mut ProgressFacts,
    aggregate: &Aggregate,
    source_locale: &str,
    target_locale: &str,
) -> OwnerResult<()> {
    let source = locale_record(aggregate, source_locale);
    if !record_complete(&source) {
        return Err(ProductCategoryFormTranslationError::IncompleteLocale {
            category_id: aggregate.id,
            locale: source_locale.to_string(),
        });
    }
    let target = locale_record(aggregate, target_locale);
    checked_add(&mut facts.resources, 1, "resources")?;
    let required_units = source.groups.len() as u64;
    checked_add(&mut facts.required_units, required_units, "required_units")?;
    let exact_required = target
        .groups
        .iter()
        .filter(|group| {
            group
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
    if exact_required == required_units {
        checked_add(&mut facts.complete_resources, 1, "complete_resources")?;
    }
    Ok(())
}

fn summary_from_owner(owner: &ExactLocaleSnapshot) -> Result<TranslationResourceSummary, PortError> {
    Ok(TranslationResourceSummary {
        identity: category_form_identity(owner.category_id),
        display_label: owner.category_code.clone(),
        lifecycle: if owner.active {
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
                        "product.category_form_translation_locale_invalid",
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
            "product.category_form_translation_locale_identity_invalid",
            "Product Category Form owner returned different exact locales",
        ));
    }
    let target = owner.target.as_ref();
    let mut fields = Vec::with_capacity(owner.source.groups.len());
    for source_group in &owner.source.groups {
        let source = source_group.label.as_deref().ok_or_else(|| {
            PortError::invariant_violation(
                "product.category_form_translation_source_invalid",
                "Product Category Form source group label is missing",
            )
        })?;
        let target_value = target
            .and_then(|target| {
                target
                    .groups
                    .iter()
                    .find(|group| group.group_id == source_group.group_id)
            })
            .and_then(|group| group.label.clone());
        fields.push(text_field(
            &format!("{GROUP_LABEL_PREFIX}{}", source_group.group_id),
            source,
            target_value,
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
            "product.category_form_translation_snapshot_invalid",
            error.to_string(),
        )
    })?;
    Ok(snapshot)
}

fn text_field(
    key: &str,
    source_value: &str,
    exact_target_value: Option<String>,
) -> Result<TranslationFieldSnapshot, PortError> {
    Ok(TranslationFieldSnapshot {
        descriptor: TranslationFieldDescriptor {
            key: FieldKey::new(key).map_err(|error| {
                PortError::invariant_violation(
                    "product.category_form_translation_field_key_invalid",
                    error.to_string(),
                )
            })?,
            profile: TranslationValueProfile::PlainText,
            strategy: TranslationStrategy::Translate,
            classification: TranslationDataClassification::Public,
            required: true,
            ai_export_allowed: true,
            max_characters: Some(255),
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
    let mut groups = Vec::with_capacity(owner.source.groups.len());
    for source_group in &owner.source.groups {
        let key = format!("{GROUP_LABEL_PREFIX}{}", source_group.group_id);
        let label = required_target_value(
            values.remove(key.as_str()).flatten(),
            "category form group label",
        )?;
        groups.push(GroupApply {
            group_id: source_group.group_id,
            label,
        });
    }
    if !values.is_empty() {
        return Err(PortError::validation(
            "product.category_form_translation_patch_field_unknown",
            "Product Category Form translation patch contains an unknown field",
        ));
    }
    Ok(ExactLocaleApply {
        source_locale: request.source_locale.as_str().to_string(),
        target_locale: request.target_locale.as_str().to_string(),
        groups,
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
            "product.category_form_translation_receipt_identity_missing",
            "Product Category Form translation owner receipt is missing operation identity",
        )
    })?;
    Ok(TranslationApplicationReceipt {
        provider_receipt_id: format!("product-category-form:{operation_id}"),
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
            "Product Category Form translation target context must carry a UUID tenant_id",
        )
    })
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Products, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "product.category_form_translation_permission_denied",
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
            "product.category_form_translation_identity_invalid",
            "Product Category Form translation identity must address product/category_form without a subresource",
        ));
    }
    Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "product.category_form_translation_resource_id_invalid",
            "Product Category Form translation resource id must be a UUID",
        )
    })
}

fn category_form_identity(category_id: Uuid) -> TranslationResourceIdentity {
    TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(TRANSLATION_OWNER_SLUG)
            .expect("static Product owner slug must satisfy Translation contract"),
        resource_kind: ResourceKind::new(TRANSLATION_RESOURCE_KIND)
            .expect("static Product Category Form resource kind must satisfy Translation contract"),
        resource_id: ResourceId::new(category_id.to_string())
            .expect("Product Category UUID must satisfy Translation resource id contract"),
        subresource_id: None,
    }
}

fn resource_cursor(category_id: Uuid) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(category_id.to_string()).map_err(|error| {
        PortError::invariant_violation(
            "product.category_form_translation_resource_cursor_invalid",
            error.to_string(),
        )
    })
}

fn parse_resource_cursor(value: &str) -> Result<Uuid, PortError> {
    Uuid::parse_str(value).map_err(|_| {
        PortError::validation(
            "product.category_form_translation_resource_cursor_invalid",
            "Product Category Form translation resource cursor must be a UUID",
        )
    })
}

fn change_cursor(through: u64, after: u64) -> Result<OpaqueCursor, PortError> {
    OpaqueCursor::new(format!("{CHANGE_CURSOR_VERSION}:{through}:{after}")).map_err(|error| {
        PortError::invariant_violation(
            "product.category_form_translation_change_cursor_invalid",
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
            "product.category_form_translation_change_cursor_invalid",
            "Product Category Form translation change cursor is invalid",
        ));
    }
    let through = through.unwrap_or_default();
    let after = after.unwrap_or_default();
    if through == 0 || after == 0 || after > through {
        return Err(PortError::validation(
            "product.category_form_translation_change_cursor_invalid",
            "Product Category Form translation change cursor bounds are invalid",
        ));
    }
    Ok((through, after))
}

fn opaque_revision(value: String, field: &'static str) -> Result<OpaqueRevision, PortError> {
    OpaqueRevision::new(value).map_err(|error| {
        PortError::invariant_violation(
            "product.category_form_translation_revision_invalid",
            format!("Product Category Form {field} is invalid: {error}"),
        )
    })
}

fn owner_error_to_port_error(error: ProductCategoryFormTranslationError) -> PortError {
    match error {
        ProductCategoryFormTranslationError::CategoryFormNotFound(_) => PortError::not_found(
            "product.category_form_translation_resource_not_found",
            "Product Category Form translation resource was not found",
        ),
        ProductCategoryFormTranslationError::SourceLocaleNotFound { .. } => PortError::not_found(
            "product.category_form_translation_source_not_found",
            "Exact source Product Category Form locale was not found",
        ),
        ProductCategoryFormTranslationError::IncompleteLocale { .. }
        | ProductCategoryFormTranslationError::OwnerInvariant(_) => PortError::invariant_violation(
            "product.category_form_translation_owner_invariant",
            "Product Category Form translation owner state is invalid",
        ),
        ProductCategoryFormTranslationError::RevisionConflict { .. }
        | ProductCategoryFormTranslationError::GroupSetMismatch => PortError::conflict(
            "product.category_form_translation_revision_conflict",
            "Product Category Form translation state conflicts with the requested mutation",
        ),
        ProductCategoryFormTranslationError::Commerce(error) => product_error_to_port_error(error),
    }
}

fn product_error_to_port_error(error: CommerceError) -> PortError {
    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "product.category_form_translation_owner_unavailable",
            "Product Category Form translation storage is temporarily unavailable",
        ),
        CommerceError::ProductNotFound(_) => PortError::not_found(
            "product.category_form_translation_resource_not_found",
            "Product Category Form translation resource was not found",
        ),
        CommerceError::DuplicateHandle { .. } | CommerceError::DuplicateSku(_) => {
            PortError::conflict(
                "product.category_form_translation_owner_conflict",
                "Product state conflicts with the requested Category Form translation mutation",
            )
        }
        CommerceError::Validation(_) | CommerceError::NoVariants => PortError::validation(
            "product.category_form_translation_owner_validation",
            "Product rejected the Category Form translation mutation",
        ),
        CommerceError::CannotDeletePublished => PortError::conflict(
            "product.category_form_translation_owner_conflict",
            "Product state conflicts with the requested Category Form translation mutation",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "product.category_form_translation_owner_invariant",
            "Product Category Form translation state is invalid",
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
            "Product category form translation source and target locale must differ".to_string(),
        )
        .into());
    }
    Ok(())
}

fn validate_stored_group_translation(translation: &StoredGroupTranslation) -> OwnerResult<()> {
    if canonical_locale(&translation.locale)? != translation.locale
        || translation.label.trim().is_empty()
        || translation.label.trim() != translation.label
        || translation.label.chars().count() > 255
    {
        return Err(ProductCategoryFormTranslationError::OwnerInvariant(
            "persisted Product category form group translation violates its owner contract"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_target_value(value: &str, label: &str, max: Option<usize>) -> OwnerResult<()> {
    if value.trim().is_empty()
        || value.trim() != value
        || max.is_some_and(|max| value.chars().count() > max)
    {
        return Err(CommerceError::Validation(format!(
            "Product category form translation {label} must be normalized, nonblank, and within its length limit"
        ))
        .into());
    }
    Ok(())
}

fn validate_uuid(value: Uuid, field: &str) -> OwnerResult<()> {
    if value.is_nil() {
        return Err(CommerceError::Validation(format!(
            "Product category form translation {field} must not be nil"
        ))
        .into());
    }
    Ok(())
}

fn ensure_revision(revision: &'static str, expected: &str, current: &str) -> OwnerResult<()> {
    if expected != current {
        return Err(ProductCategoryFormTranslationError::RevisionConflict { revision });
    }
    Ok(())
}

fn ensure_postgres(backend: DatabaseBackend) -> OwnerResult<()> {
    if backend != DatabaseBackend::Postgres {
        return Err(CommerceError::Validation(
            "Product category form Translation target requires PostgreSQL".to_string(),
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

fn invalid_sequence(field: &str) -> ProductCategoryFormTranslationError {
    ProductCategoryFormTranslationError::OwnerInvariant(format!(
        "Product category form translation change {field} sequence must be positive"
    ))
}

fn change_record_from_row(row: sea_orm::QueryResult) -> OwnerResult<ChangeRecord> {
    let change_seq = positive_sequence(row.try_get("", "change_seq")?, "change")?;
    let category_id: Uuid = row.try_get("", "category_id")?;
    let resource_revision: String = row.try_get("", "resource_revision")?;
    let lifecycle: String = row.try_get("", "lifecycle")?;
    if category_id.is_nil() || resource_revision.trim().is_empty() {
        return Err(ProductCategoryFormTranslationError::OwnerInvariant(
            "Product category form translation change row is invalid".to_string(),
        ));
    }
    Ok(ChangeRecord {
        change_seq,
        category_id,
        resource_revision,
        lifecycle: ChangeLifecycle::parse(&lifecycle)?,
    })
}

fn owner_error_to_commerce(error: ProductCategoryFormTranslationError) -> CommerceError {
    match error {
        ProductCategoryFormTranslationError::Commerce(error) => error,
        error => CommerceError::Validation(error.to_string()),
    }
}

fn checked_add(value: &mut u64, increment: u64, label: &str) -> OwnerResult<()> {
    *value = value.checked_add(increment).ok_or_else(|| overflow(label))?;
    Ok(())
}

fn overflow(label: &str) -> ProductCategoryFormTranslationError {
    ProductCategoryFormTranslationError::OwnerInvariant(format!(
        "Product category form translation progress `{label}` overflowed u64"
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
