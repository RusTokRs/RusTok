use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use flex::{
    FlexAttachedLocalizedValuesByEntity, FlexAttachedTranslationError,
    FlexAttachedTranslationExactLocaleApply, FlexAttachedTranslationExactLocaleApplyReceipt,
    FlexAttachedTranslationExactLocaleSnapshot, FlexAttachedTranslationLeaf,
    FlexAttachedTranslationLeafSnapshot, FlexAttachedTranslationOwnerPort,
    FlexAttachedTranslationResourcePage, FlexAttachedTranslationResourceRevisionsByEntity,
    FlexAttachedTranslationResult, FlexAttachedTranslationTargetValue,
    TAXONOMY_CATEGORY_ENTITY_TYPE, flex_attached_translation_field_eligible,
    load_attached_translation_localized_values, load_attached_translation_resource_revisions,
    load_attached_translation_schema_in, lock_attached_translation_schema_in_tx,
    persist_localized_values, validate_flex_attached_translation_locale_pair,
    validate_flex_attached_translation_resource_page,
};
use rustok_api::PortError;
use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FlexError};
use rustok_events::DomainEvent;
use rustok_outbox::{TransactionalEventBus, idempotency};
use sea_orm::{AccessMode, DatabaseConnection, IsolationLevel, TransactionTrait};
use serde::Serialize;
use serde_json::{Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const OWNER_SLUG: &str = "flex";
const RESOURCE_KIND: &str = "attached_localized_value";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_attached_value_patch";
const LOCALE_REVISION_NAMESPACE: &str = "rustok-flex/attached-locale/v1";

#[derive(Serialize)]
struct OwnerApplyRequestHash<'a> {
    entity_type: &'a str,
    entity_id: Uuid,
    idempotency_key: &'a str,
    proposal_id: &'a str,
    approval_receipt_id: &'a str,
    request_fingerprint: &'a str,
    source_locale: &'a str,
    target_locale: &'a str,
    expected_resource_revision: &'a str,
    expected_source_revision: &'a str,
    expected_target_revision: Option<&'a str>,
}

#[derive(Clone)]
pub struct ServerFlexTaxonomyCategoryTranslationOwner {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ServerFlexTaxonomyCategoryTranslationOwner {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    async fn apply_under_lease(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        entity_id: Uuid,
        request: FlexAttachedTranslationExactLocaleApply,
        lease: idempotency::Lease,
    ) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleApplyReceipt> {
        let txn = self.db.begin().await.map_err(database_error)?;

        // Lock order is shared with the canonical attached GraphQL writer:
        // Flex schema generation first, then the Taxonomy Category owner row.
        let schema =
            lock_attached_translation_schema_in_tx(&txn, tenant_id, TAXONOMY_CATEGORY_ENTITY_TYPE)
                .await
                .map_err(flex_storage_error)?
                .schema;
        let owner = rustok_taxonomy::lock_category_owner_revision_in_tx(&txn, tenant_id, entity_id)
            .await
            .map_err(|error| taxonomy_owner_error(error, entity_id))?;
        let before_values = load_attached_translation_localized_values(
            &txn,
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            &[entity_id],
        )
        .await
        .map_err(flex_storage_error)?;
        let before_revisions = load_attached_translation_resource_revisions(
            &txn,
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            &[entity_id],
        )
        .await
        .map_err(flex_storage_error)?;
        let before = build_snapshot_from_batch(
            tenant_id,
            entity_id,
            &schema,
            &before_values,
            &before_revisions,
            &request.source_locale,
            &request.target_locale,
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
            return Err(FlexAttachedTranslationError::RevisionConflict { revision: "target" });
        }
        request.validate()?;

        let requested = request
            .target_values
            .iter()
            .cloned()
            .map(|target| (target.leaf, target.value))
            .collect::<BTreeMap<_, _>>();
        let source_leaves = before
            .leaves
            .iter()
            .map(|leaf| leaf.leaf.clone())
            .collect::<BTreeSet<_>>();
        let requested_leaves = requested.keys().cloned().collect::<BTreeSet<_>>();
        if source_leaves != requested_leaves {
            return Err(FlexAttachedTranslationError::Invalid(
                "Flex attached translation apply must contain exactly the source-visible leaf set"
                    .to_string(),
            ));
        }
        validate_requested_targets(&schema, &requested)?;

        let current_by_locale = before_values.get(&entity_id).cloned().unwrap_or_default();
        let mut desired_target = current_by_locale
            .get(&request.target_locale)
            .cloned()
            .unwrap_or_default();
        for (leaf, value) in &requested {
            match value {
                Some(value) => {
                    desired_target.insert(leaf.field_key.clone(), JsonValue::String(value.clone()));
                }
                None => {
                    desired_target.remove(&leaf.field_key);
                }
            }
        }
        let current_target = current_by_locale
            .get(&request.target_locale)
            .cloned()
            .unwrap_or_default();
        let changed = current_target != desired_target;

        if changed {
            persist_localized_values(
                &txn,
                tenant_id,
                TAXONOMY_CATEGORY_ENTITY_TYPE,
                entity_id,
                &request.target_locale,
                &JsonValue::Object(desired_target),
            )
            .await
            .map_err(flex_storage_error)?;
            rustok_taxonomy::advance_category_owner_revision_in_tx(
                &txn,
                tenant_id,
                entity_id,
                owner.revision,
            )
            .await
            .map_err(|error| taxonomy_owner_error(error, entity_id))?;
        }

        let after_values = load_attached_translation_localized_values(
            &txn,
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            &[entity_id],
        )
        .await
        .map_err(flex_storage_error)?;
        let after_revisions = load_attached_translation_resource_revisions(
            &txn,
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            &[entity_id],
        )
        .await
        .map_err(flex_storage_error)?;
        let after = build_snapshot_from_batch(
            tenant_id,
            entity_id,
            &schema,
            &after_values,
            &after_revisions,
            &request.source_locale,
            &request.target_locale,
        )?;
        verify_applied_targets(&requested, &after)?;

        // A successful patch may remove the last optional exact target leaf. The neutral
        // application receipt still requires a post-apply revision token, so the receipt
        // hashes the exact empty target state even though subsequent reads correctly expose
        // `target_revision=None` to mean that no exact target locale currently exists.
        let after_target_values = exact_values_for_locale(
            &schema,
            after_values
                .get(&entity_id)
                .and_then(|locales| locales.get(&request.target_locale)),
            &request.target_locale,
        )?;
        let receipt_target_revision = locale_revision(
            tenant_id,
            entity_id,
            &request.target_locale,
            &after_target_values,
        );

        if changed {
            self.event_bus
                .publish_in_tx_with_envelope_id(
                    &txn,
                    tenant_id,
                    actor_user_id,
                    DomainEvent::TranslationTargetChanged {
                        owner_slug: OWNER_SLUG.to_string(),
                        resource_kind: RESOURCE_KIND.to_string(),
                        resource_id: entity_id.to_string(),
                        changed_locale: request.target_locale.clone(),
                        resource_revision: after.resource_revision.clone(),
                        target_revision: receipt_target_revision.clone(),
                        operation: "apply_exact_locale".to_string(),
                        correlation_id: lease.operation_id.to_string(),
                    },
                )
                .await
                .map_err(|error| {
                    FlexAttachedTranslationError::OwnerInvariant(format!(
                        "failed to publish attached Translation owner event: {error}"
                    ))
                })?;
        }

        let receipt = FlexAttachedTranslationExactLocaleApplyReceipt {
            operation_id: lease.operation_id,
            entity_type: TAXONOMY_CATEGORY_ENTITY_TYPE.to_string(),
            entity_id,
            resource_revision: after.resource_revision,
            target_revision: receipt_target_revision,
            target_values: requested
                .into_iter()
                .map(|(leaf, value)| FlexAttachedTranslationTargetValue { leaf, value })
                .collect(),
        };
        idempotency::complete(&txn, lease, &receipt)
            .await
            .map_err(FlexAttachedTranslationError::Operation)?;
        txn.commit().await.map_err(database_error)?;
        Ok(receipt)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &FlexAttachedTranslationError) {
        let port_error = owner_error_to_port_error(error);
        if let Err(receipt_error) = idempotency::fail(&self.db, lease, &port_error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist attached Translation owner failure receipt"
            );
        }
    }
}

#[async_trait]
impl FlexAttachedTranslationOwnerPort for ServerFlexTaxonomyCategoryTranslationOwner {
    fn entity_type(&self) -> &str {
        TAXONOMY_CATEGORY_ENTITY_TYPE
    }

    async fn list_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<Uuid>,
        limit: u16,
    ) -> FlexAttachedTranslationResult<FlexAttachedTranslationResourcePage> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_flex_attached_translation_locale_pair(source_locale, target_locale)?;
        validate_flex_attached_translation_resource_page(limit)?;
        if let Some(after) = after {
            validate_uuid(after, "after")?;
        }

        let txn = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
            .map_err(database_error)?;
        let schema =
            load_attached_translation_schema_in(&txn, tenant_id, TAXONOMY_CATEGORY_ENTITY_TYPE)
                .await
                .map_err(flex_storage_error)?;
        let page = rustok_taxonomy::list_category_owner_revisions_in(&txn, tenant_id, after, limit)
            .await
            .map_err(taxonomy_inventory_error)?;
        let ids = page
            .categories
            .iter()
            .map(|category| category.category_id)
            .collect::<Vec<_>>();
        let values = load_attached_translation_localized_values(
            &txn,
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            &ids,
        )
        .await
        .map_err(flex_storage_error)?;
        let revisions = load_attached_translation_resource_revisions(
            &txn,
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            &ids,
        )
        .await
        .map_err(flex_storage_error)?;

        let mut resources = Vec::with_capacity(page.categories.len());
        for category in page.categories {
            match build_snapshot_from_batch(
                tenant_id,
                category.category_id,
                &schema,
                &values,
                &revisions,
                source_locale,
                target_locale,
            ) {
                Ok(snapshot) => resources.push(snapshot),
                Err(FlexAttachedTranslationError::SourceLocaleNotFound { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        let result = FlexAttachedTranslationResourcePage {
            resources,
            next_after: page.next_after,
        };
        txn.commit().await.map_err(database_error)?;
        Ok(result)
    }

    async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        entity_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleSnapshot> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_uuid(entity_id, "entity_id")?;
        validate_flex_attached_translation_locale_pair(source_locale, target_locale)?;

        let txn = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
            .map_err(database_error)?;
        let exists =
            rustok_taxonomy::load_category_owner_revisions_in(&txn, tenant_id, &[entity_id])
                .await
                .map_err(taxonomy_inventory_error)?
                .into_iter()
                .next()
                .is_some();
        if !exists {
            return Err(FlexAttachedTranslationError::EntityNotFound {
                entity_type: TAXONOMY_CATEGORY_ENTITY_TYPE.to_string(),
                entity_id,
            });
        }
        let schema =
            load_attached_translation_schema_in(&txn, tenant_id, TAXONOMY_CATEGORY_ENTITY_TYPE)
                .await
                .map_err(flex_storage_error)?;
        let values = load_attached_translation_localized_values(
            &txn,
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            &[entity_id],
        )
        .await
        .map_err(flex_storage_error)?;
        let revisions = load_attached_translation_resource_revisions(
            &txn,
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            &[entity_id],
        )
        .await
        .map_err(flex_storage_error)?;
        let snapshot = build_snapshot_from_batch(
            tenant_id,
            entity_id,
            &schema,
            &values,
            &revisions,
            source_locale,
            target_locale,
        )?;
        txn.commit().await.map_err(database_error)?;
        Ok(snapshot)
    }

    async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        entity_id: Uuid,
        request: FlexAttachedTranslationExactLocaleApply,
    ) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleApplyReceipt> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_uuid(entity_id, "entity_id")?;
        if let Some(actor_user_id) = actor_user_id {
            validate_uuid(actor_user_id, "actor_user_id")?;
        }
        request.validate_admission()?;

        let admission_request = OwnerApplyRequestHash {
            entity_type: TAXONOMY_CATEGORY_ENTITY_TYPE,
            entity_id,
            idempotency_key: &request.operation.idempotency_key,
            proposal_id: &request.operation.proposal_id,
            approval_receipt_id: &request.operation.approval_receipt_id,
            request_fingerprint: &request.operation.request_fingerprint,
            source_locale: &request.source_locale,
            target_locale: &request.target_locale,
            expected_resource_revision: &request.expected_resource_revision,
            expected_source_revision: &request.expected_source_revision,
            expected_target_revision: request.expected_target_revision.as_deref(),
        };
        let lease = match idempotency::admit(
            &self.db,
            idempotency::OwnerOperationScope::Tenant(tenant_id),
            OWNER_SLUG,
            &request.operation.idempotency_key,
            OPERATION_APPLY_PATCH,
            &admission_request,
        )
        .await
        .map_err(FlexAttachedTranslationError::Operation)?
        {
            idempotency::Admission::Run(lease) => lease,
            idempotency::Admission::Replay(value) => return decode_receipt(value, entity_id),
            idempotency::Admission::ReplayError(error) => {
                return Err(FlexAttachedTranslationError::Operation(error));
            }
        };

        let result = self
            .apply_under_lease(tenant_id, actor_user_id, entity_id, request, lease)
            .await;
        if let Err(error) = &result {
            self.fail_receipt(lease, error).await;
        }
        result
    }
}

pub(crate) fn build_snapshot_from_batch(
    tenant_id: Uuid,
    entity_id: Uuid,
    schema: &CustomFieldsSchema,
    batch: &FlexAttachedLocalizedValuesByEntity,
    resource_revisions: &FlexAttachedTranslationResourceRevisionsByEntity,
    source_locale: &str,
    target_locale: &str,
) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleSnapshot> {
    validate_flex_attached_translation_locale_pair(source_locale, target_locale)?;
    let localized = batch.get(&entity_id).cloned().unwrap_or_default();
    let source_values =
        exact_values_for_locale(schema, localized.get(source_locale), source_locale)?;
    if source_values.is_empty() {
        return Err(FlexAttachedTranslationError::SourceLocaleNotFound {
            entity_type: TAXONOMY_CATEGORY_ENTITY_TYPE.to_string(),
            entity_id,
            locale: source_locale.to_string(),
        });
    }
    let resource_revision = resource_revisions.get(&entity_id).cloned().ok_or_else(|| {
        FlexAttachedTranslationError::OwnerInvariant(format!(
            "attached Translation resource {entity_id} is source-visible but has no durable Flex revision"
        ))
    })?;
    let target_values =
        exact_values_for_locale(schema, localized.get(target_locale), target_locale)?;
    let required = eligible_definitions(schema)
        .into_iter()
        .map(|definition| {
            (
                FlexAttachedTranslationLeaf {
                    field_key: definition.field_key,
                },
                definition.is_required,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let leaves = source_values
        .iter()
        .map(|(leaf, source_value)| FlexAttachedTranslationLeafSnapshot {
            leaf: leaf.clone(),
            required: *required.get(leaf).unwrap_or(&false),
            source_value: source_value.clone(),
            target_value: target_values.get(leaf).cloned(),
        })
        .collect::<Vec<_>>();

    let mut exact_locales = BTreeSet::new();
    for (locale, values) in &localized {
        if !exact_values_for_locale(schema, Some(values), locale)?.is_empty() {
            exact_locales.insert(locale.clone());
        }
    }
    let snapshot = FlexAttachedTranslationExactLocaleSnapshot {
        entity_type: TAXONOMY_CATEGORY_ENTITY_TYPE.to_string(),
        entity_id,
        is_active: true,
        source_locale: source_locale.to_string(),
        target_locale: target_locale.to_string(),
        resource_revision,
        source_revision: locale_revision(tenant_id, entity_id, source_locale, &source_values),
        target_revision: (!target_values.is_empty())
            .then(|| locale_revision(tenant_id, entity_id, target_locale, &target_values)),
        exact_locales: exact_locales.into_iter().collect(),
        leaves,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

pub(crate) fn exact_values_for_locale(
    schema: &CustomFieldsSchema,
    values: Option<&Map<String, JsonValue>>,
    locale: &str,
) -> FlexAttachedTranslationResult<BTreeMap<FlexAttachedTranslationLeaf, String>> {
    let mut exact = BTreeMap::new();
    let Some(values) = values else {
        return Ok(exact);
    };
    for definition in eligible_definitions(schema) {
        let Some(value) = values.get(&definition.field_key) else {
            continue;
        };
        match value {
            JsonValue::Null => {}
            JsonValue::String(value) if value.trim().is_empty() => {
                return Err(FlexAttachedTranslationError::OwnerInvariant(format!(
                    "persisted attached Translation value is blank for {locale}/{}",
                    definition.field_key
                )));
            }
            JsonValue::String(value) => {
                exact.insert(
                    FlexAttachedTranslationLeaf {
                        field_key: definition.field_key,
                    },
                    value.clone(),
                );
            }
            _ => {
                return Err(FlexAttachedTranslationError::OwnerInvariant(format!(
                    "persisted attached Translation value is not text for {locale}/{}",
                    definition.field_key
                )));
            }
        }
    }
    Ok(exact)
}

fn eligible_definitions(schema: &CustomFieldsSchema) -> Vec<FieldDefinition> {
    let mut definitions = schema
        .active_definitions()
        .into_iter()
        .filter(|definition| flex_attached_translation_field_eligible(definition))
        .cloned()
        .collect::<Vec<_>>();
    definitions.sort_by(|left, right| left.field_key.cmp(&right.field_key));
    definitions
}

fn validate_requested_targets(
    schema: &CustomFieldsSchema,
    requested: &BTreeMap<FlexAttachedTranslationLeaf, Option<String>>,
) -> FlexAttachedTranslationResult<()> {
    let definitions = eligible_definitions(schema)
        .into_iter()
        .map(|definition| (definition.field_key.clone(), definition))
        .collect::<BTreeMap<_, _>>();
    for (leaf, value) in requested {
        let definition = definitions.get(&leaf.field_key).ok_or_else(|| {
            FlexAttachedTranslationError::Invalid(format!(
                "Flex attached translation field {} is no longer eligible",
                leaf.field_key
            ))
        })?;
        match value {
            None if definition.is_required => {
                return Err(FlexAttachedTranslationError::Invalid(format!(
                    "required attached Translation field {} cannot be removed",
                    leaf.field_key
                )));
            }
            None => {}
            Some(value) => {
                let mut object = Map::new();
                object.insert(leaf.field_key.clone(), JsonValue::String(value.clone()));
                let validation_schema = CustomFieldsSchema::new(vec![definition.clone()]);
                let errors = validation_schema.validate(&JsonValue::Object(object));
                if !errors.is_empty() {
                    return Err(FlexAttachedTranslationError::Invalid(format!(
                        "translated attached field {} violates the current Flex definition: {errors:?}",
                        leaf.field_key
                    )));
                }
            }
        }
    }
    Ok(())
}

fn locale_revision(
    tenant_id: Uuid,
    entity_id: Uuid,
    locale: &str,
    values: &BTreeMap<FlexAttachedTranslationLeaf, String>,
) -> String {
    let mut hasher = Sha256::new();
    hash_str(&mut hasher, LOCALE_REVISION_NAMESPACE);
    hash_uuid(&mut hasher, tenant_id);
    hash_uuid(&mut hasher, entity_id);
    hash_str(&mut hasher, TAXONOMY_CATEGORY_ENTITY_TYPE);
    hash_str(&mut hasher, locale);
    hash_len(&mut hasher, values.len());
    for (leaf, value) in values {
        hash_str(&mut hasher, &leaf.field_key);
        hash_str(&mut hasher, value);
    }
    format!("flex-attached-locale-v1:{}", hex::encode(hasher.finalize()))
}

fn hash_str(hasher: &mut Sha256, value: &str) {
    hash_len(hasher, value.len());
    hasher.update(value.as_bytes());
}

fn hash_uuid(hasher: &mut Sha256, value: Uuid) {
    hasher.update(value.as_bytes());
}

fn hash_len(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_be_bytes());
}

fn ensure_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> FlexAttachedTranslationResult<()> {
    if expected != current {
        return Err(FlexAttachedTranslationError::RevisionConflict { revision });
    }
    Ok(())
}

fn verify_applied_targets(
    requested: &BTreeMap<FlexAttachedTranslationLeaf, Option<String>>,
    snapshot: &FlexAttachedTranslationExactLocaleSnapshot,
) -> FlexAttachedTranslationResult<()> {
    let actual = snapshot
        .leaves
        .iter()
        .map(|leaf| (leaf.leaf.clone(), leaf.target_value.clone()))
        .collect::<BTreeMap<_, _>>();
    if &actual != requested {
        return Err(FlexAttachedTranslationError::OwnerInvariant(
            "attached exact target state does not match the applied target set".to_string(),
        ));
    }
    Ok(())
}

fn validate_uuid(value: Uuid, label: &str) -> FlexAttachedTranslationResult<()> {
    if value.is_nil() {
        return Err(FlexAttachedTranslationError::Invalid(format!(
            "Flex attached translation {label} must not be the nil UUID"
        )));
    }
    Ok(())
}

fn decode_receipt(
    value: JsonValue,
    expected_entity_id: Uuid,
) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleApplyReceipt> {
    let receipt: FlexAttachedTranslationExactLocaleApplyReceipt = serde_json::from_value(value)
        .map_err(|error| {
            FlexAttachedTranslationError::Operation(PortError::invariant_violation(
                "outbox.operation_receipt_corrupt",
                error.to_string(),
            ))
        })?;
    let mut leaves = BTreeSet::new();
    let invalid = receipt.entity_type != TAXONOMY_CATEGORY_ENTITY_TYPE
        || receipt.entity_id != expected_entity_id
        || receipt.operation_id.is_nil()
        || receipt.resource_revision.trim().is_empty()
        || receipt.target_revision.trim().is_empty()
        || receipt.target_values.is_empty()
        || receipt.target_values.iter().any(|target| {
            target.leaf.validate().is_err()
                || !leaves.insert(target.leaf.clone())
                || target
                    .value
                    .as_ref()
                    .is_some_and(|value| value.trim().is_empty())
        });
    if invalid {
        return Err(FlexAttachedTranslationError::Operation(
            PortError::invariant_violation(
                "outbox.operation_receipt_corrupt",
                "attached Translation owner receipt violates its identity contract",
            ),
        ));
    }
    Ok(receipt)
}

fn taxonomy_inventory_error(error: rustok_taxonomy::TaxonomyError) -> FlexAttachedTranslationError {
    match error {
        rustok_taxonomy::TaxonomyError::Database(error) => {
            FlexAttachedTranslationError::Storage(error.to_string())
        }
        error => FlexAttachedTranslationError::OwnerInvariant(format!(
            "Taxonomy Category inventory violates attached owner contract: {error}"
        )),
    }
}

fn taxonomy_owner_error(
    error: rustok_taxonomy::TaxonomyError,
    entity_id: Uuid,
) -> FlexAttachedTranslationError {
    match error {
        rustok_taxonomy::TaxonomyError::TermNotFound(_) => {
            FlexAttachedTranslationError::EntityNotFound {
                entity_type: TAXONOMY_CATEGORY_ENTITY_TYPE.to_string(),
                entity_id,
            }
        }
        rustok_taxonomy::TaxonomyError::Conflict(_) => {
            FlexAttachedTranslationError::RevisionConflict {
                revision: "resource",
            }
        }
        rustok_taxonomy::TaxonomyError::Database(error) => {
            FlexAttachedTranslationError::Storage(error.to_string())
        }
        error => FlexAttachedTranslationError::OwnerInvariant(format!(
            "Taxonomy Category owner mutation violates attached contract: {error}"
        )),
    }
}

fn flex_storage_error(error: FlexError) -> FlexAttachedTranslationError {
    FlexAttachedTranslationError::Storage(error.to_string())
}

fn database_error(error: sea_orm::DbErr) -> FlexAttachedTranslationError {
    FlexAttachedTranslationError::Storage(error.to_string())
}

fn owner_error_to_port_error(error: &FlexAttachedTranslationError) -> PortError {
    match error {
        FlexAttachedTranslationError::Invalid(_) => PortError::validation(
            "flex.attached_translation_owner_validation",
            "Flex rejected the attached translation request",
        ),
        FlexAttachedTranslationError::EntityNotFound { .. } => PortError::not_found(
            "flex.attached_translation_resource_not_found",
            "Attached Flex donor resource was not found",
        ),
        FlexAttachedTranslationError::SourceLocaleNotFound { .. } => PortError::not_found(
            "flex.attached_translation_source_not_found",
            "Exact source attached Flex locale was not found",
        ),
        FlexAttachedTranslationError::RevisionConflict { .. } => PortError::conflict(
            "flex.attached_translation_revision_conflict",
            "Attached Flex translation state conflicts with the request",
        ),
        FlexAttachedTranslationError::Operation(error) => error.clone(),
        FlexAttachedTranslationError::Storage(_) => PortError::unavailable(
            "flex.attached_translation_owner_unavailable",
            "Attached Flex translation storage is temporarily unavailable",
        ),
        FlexAttachedTranslationError::OwnerInvariant(_) => PortError::invariant_violation(
            "flex.attached_translation_owner_invariant",
            "Attached Flex translation owner state is invalid",
        ),
    }
}
