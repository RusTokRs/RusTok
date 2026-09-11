use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use flex::{
    FlexStandaloneTranslationError, FlexStandaloneTranslationExactLocaleApply,
    FlexStandaloneTranslationExactLocaleApplyReceipt, FlexStandaloneTranslationExactLocaleSnapshot,
    FlexStandaloneTranslationExactProgress, FlexStandaloneTranslationLeaf,
    FlexStandaloneTranslationLeafSnapshot, FlexStandaloneTranslationOwnerPort,
    FlexStandaloneTranslationProgressOwnerPort, FlexStandaloneTranslationResourcePage,
    FlexStandaloneTranslationResult, FlexStandaloneTranslationTargetValue,
    flex_standalone_translation_field_eligible, parse_standalone_fields_config,
    validate_flex_standalone_translation_locale_pair,
    validate_flex_standalone_translation_resource_page,
};
use rustok_api::PortError;
use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition};
use rustok_events::DomainEvent;
use rustok_outbox::{TransactionalEventBus, idempotency};
use sea_orm::{
    AccessMode, ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait,
    DatabaseBackend, DatabaseConnection, EntityTrait, FromQueryResult, IsolationLevel, QueryFilter,
    QueryOrder, QuerySelect, Statement, TransactionTrait,
};
use serde::Serialize;
use serde_json::{Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::models::{flex_entries, flex_entry_localized_values, flex_schemas};

const OWNER_SLUG: &str = "flex";
const RESOURCE_KIND: &str = "standalone_localized_value";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_standalone_value_patch";
const LOCALE_REVISION_NAMESPACE: &str = "rustok-flex/standalone-locale/v1";
const PROGRESS_PAGE_SIZE: u16 = 200;

#[derive(Serialize)]
struct OwnerApplyRequestHash<'a> {
    schema_id: Uuid,
    entry_id: Uuid,
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

#[derive(Debug, FromQueryResult)]
struct RevisionRow {
    revision: i64,
}

#[derive(Clone)]
pub struct ServerFlexStandaloneTranslationOwner {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ServerFlexStandaloneTranslationOwner {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    async fn apply_under_lease(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        schema_id: Uuid,
        entry_id: Uuid,
        request: FlexStandaloneTranslationExactLocaleApply,
        lease: idempotency::Lease,
    ) -> FlexStandaloneTranslationResult<FlexStandaloneTranslationExactLocaleApplyReceipt> {
        // Standalone source/schema changes advance the durable resource state from triggers. A
        // serializable transaction makes that state row the CAS serialization point without
        // introducing a lock-order cycle against concurrent localized-row writers.
        let txn = self
            .db
            .begin_with_config(Some(IsolationLevel::Serializable), None)
            .await
            .map_err(database_error)?;

        txn.query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT id FROM flex_entries WHERE tenant_id = $1 AND schema_id = $2 AND id = $3 FOR UPDATE",
            vec![tenant_id.into(), schema_id.into(), entry_id.into()],
        ))
        .await
        .map_err(database_error)?
        .ok_or(FlexStandaloneTranslationError::EntryNotFound { schema_id, entry_id })?;

        let before = build_snapshot_in(
            &txn,
            tenant_id,
            schema_id,
            entry_id,
            &request.source_locale,
            &request.target_locale,
        )
        .await?;
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
            return Err(FlexStandaloneTranslationError::RevisionConflict { revision: "target" });
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
            return Err(FlexStandaloneTranslationError::Invalid(
                "Flex standalone translation apply must contain exactly the source-visible leaf set"
                    .to_string(),
            ));
        }

        let schema = flex_schemas::Entity::find_by_id(schema_id)
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await
            .map_err(database_error)?
            .ok_or(FlexStandaloneTranslationError::EntryNotFound { schema_id, entry_id })?;
        let definitions = eligible_definitions(schema.fields_config.clone())?;
        validate_requested_targets(&definitions, &requested)?;

        let existing = flex_entry_localized_values::Entity::find()
            .filter(flex_entry_localized_values::Column::TenantId.eq(tenant_id))
            .filter(flex_entry_localized_values::Column::EntryId.eq(entry_id))
            .filter(flex_entry_localized_values::Column::Locale.eq(&request.target_locale))
            .one(&txn)
            .await
            .map_err(database_error)?;
        let mut desired = existing
            .as_ref()
            .and_then(|row| row.data.as_object().cloned())
            .unwrap_or_default();
        for (leaf, value) in &requested {
            match value {
                Some(value) => {
                    desired.insert(leaf.field_key.clone(), JsonValue::String(value.clone()));
                }
                None => {
                    desired.remove(&leaf.field_key);
                }
            }
        }
        let current = existing
            .as_ref()
            .and_then(|row| row.data.as_object().cloned())
            .unwrap_or_default();
        let changed = current != desired;

        if changed {
            if desired.is_empty() {
                if let Some(row) = existing {
                    let model: flex_entry_localized_values::ActiveModel = row.into();
                    model.delete(&txn).await.map_err(database_error)?;
                }
            } else if let Some(row) = existing {
                let mut model: flex_entry_localized_values::ActiveModel = row.into();
                model.data = Set(JsonValue::Object(desired.clone()));
                model.update(&txn).await.map_err(database_error)?;
            } else {
                flex_entry_localized_values::ActiveModel {
                    entry_id: Set(entry_id),
                    locale: Set(request.target_locale.clone()),
                    tenant_id: Set(tenant_id),
                    data: Set(JsonValue::Object(desired.clone())),
                    created_at: sea_orm::ActiveValue::NotSet,
                    updated_at: sea_orm::ActiveValue::NotSet,
                }
                .insert(&txn)
                .await
                .map_err(database_error)?;
            }
        }

        let after = build_snapshot_in(
            &txn,
            tenant_id,
            schema_id,
            entry_id,
            &request.source_locale,
            &request.target_locale,
        )
        .await?;
        verify_applied_targets(&requested, &after)?;
        let after_target_values = exact_values_for_locale(
            &definitions,
            Some(&JsonValue::Object(desired)),
            &request.target_locale,
        )?;
        let receipt_target_revision = locale_revision(
            tenant_id,
            schema_id,
            entry_id,
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
                        resource_id: entry_id.to_string(),
                        changed_locale: request.target_locale.clone(),
                        resource_revision: after.resource_revision.clone(),
                        target_revision: receipt_target_revision.clone(),
                        operation: "apply_exact_locale".to_string(),
                        correlation_id: lease.operation_id.to_string(),
                    },
                )
                .await
                .map_err(|error| {
                    FlexStandaloneTranslationError::OwnerInvariant(format!(
                        "failed to publish standalone Translation owner event: {error}"
                    ))
                })?;
        }

        let receipt = FlexStandaloneTranslationExactLocaleApplyReceipt {
            operation_id: lease.operation_id,
            schema_id,
            entry_id,
            resource_revision: after.resource_revision,
            target_revision: receipt_target_revision,
            target_values: requested
                .into_iter()
                .map(|(leaf, value)| FlexStandaloneTranslationTargetValue { leaf, value })
                .collect(),
        };
        idempotency::complete(&txn, lease, &receipt)
            .await
            .map_err(FlexStandaloneTranslationError::Operation)?;
        txn.commit().await.map_err(database_error)?;
        Ok(receipt)
    }

    async fn fail_receipt(
        &self,
        lease: idempotency::Lease,
        error: &FlexStandaloneTranslationError,
    ) {
        let port_error = owner_error_to_port_error(error);
        if let Err(receipt_error) = idempotency::fail(&self.db, lease, &port_error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist standalone Translation owner failure receipt"
            );
        }
    }
}

#[async_trait]
impl FlexStandaloneTranslationOwnerPort for ServerFlexStandaloneTranslationOwner {
    async fn list_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<Uuid>,
        limit: u16,
    ) -> FlexStandaloneTranslationResult<FlexStandaloneTranslationResourcePage> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_flex_standalone_translation_locale_pair(source_locale, target_locale)?;
        validate_flex_standalone_translation_resource_page(limit)?;
        if let Some(after) = after {
            validate_uuid(after, "after")?;
        }
        ensure_postgres(&self.db)?;

        let txn = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
            .map_err(database_error)?;
        let mut query = flex_entries::Entity::find()
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .order_by_asc(flex_entries::Column::Id)
            .limit(u64::from(limit));
        if let Some(after) = after {
            query = query.filter(flex_entries::Column::Id.gt(after));
        }
        let rows = query.all(&txn).await.map_err(database_error)?;
        let next_after = (rows.len() == usize::from(limit))
            .then(|| rows.last().map(|row| row.id))
            .flatten();
        let mut resources = Vec::with_capacity(rows.len());
        for row in rows {
            match build_snapshot_in(
                &txn,
                tenant_id,
                row.schema_id,
                row.id,
                source_locale,
                target_locale,
            )
            .await
            {
                Ok(snapshot) => resources.push(snapshot),
                Err(FlexStandaloneTranslationError::SourceLocaleNotFound { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        txn.commit().await.map_err(database_error)?;
        Ok(FlexStandaloneTranslationResourcePage {
            resources,
            next_after,
        })
    }

    async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
        entry_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexStandaloneTranslationResult<FlexStandaloneTranslationExactLocaleSnapshot> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_uuid(schema_id, "schema_id")?;
        validate_uuid(entry_id, "entry_id")?;
        validate_flex_standalone_translation_locale_pair(source_locale, target_locale)?;
        ensure_postgres(&self.db)?;
        let txn = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
            .map_err(database_error)?;
        let snapshot = build_snapshot_in(
            &txn,
            tenant_id,
            schema_id,
            entry_id,
            source_locale,
            target_locale,
        )
        .await?;
        txn.commit().await.map_err(database_error)?;
        Ok(snapshot)
    }

    async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        schema_id: Uuid,
        entry_id: Uuid,
        request: FlexStandaloneTranslationExactLocaleApply,
    ) -> FlexStandaloneTranslationResult<FlexStandaloneTranslationExactLocaleApplyReceipt> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_uuid(schema_id, "schema_id")?;
        validate_uuid(entry_id, "entry_id")?;
        if let Some(actor_user_id) = actor_user_id {
            validate_uuid(actor_user_id, "actor_user_id")?;
        }
        ensure_postgres(&self.db)?;
        request.validate_admission()?;

        let admission_request = OwnerApplyRequestHash {
            schema_id,
            entry_id,
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
        .map_err(FlexStandaloneTranslationError::Operation)?
        {
            idempotency::Admission::Run(lease) => lease,
            idempotency::Admission::Replay(value) => {
                return decode_receipt(value, schema_id, entry_id);
            }
            idempotency::Admission::ReplayError(error) => {
                return Err(FlexStandaloneTranslationError::Operation(error));
            }
        };

        let result = self
            .apply_under_lease(
                tenant_id,
                actor_user_id,
                schema_id,
                entry_id,
                request,
                lease,
            )
            .await;
        if let Err(error) = &result {
            self.fail_receipt(lease, error).await;
        }
        result
    }
}

#[async_trait]
impl FlexStandaloneTranslationProgressOwnerPort for ServerFlexStandaloneTranslationOwner {
    async fn read_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexStandaloneTranslationResult<FlexStandaloneTranslationExactProgress> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_flex_standalone_translation_locale_pair(source_locale, target_locale)?;
        ensure_postgres(&self.db)?;

        let mut progress = FlexStandaloneTranslationExactProgress::default();
        let mut after = None;
        loop {
            let page = self
                .list_exact_resources(
                    tenant_id,
                    source_locale,
                    target_locale,
                    after,
                    PROGRESS_PAGE_SIZE,
                )
                .await?;
            for snapshot in &page.resources {
                observe_snapshot(&mut progress, snapshot)?;
            }
            let Some(next_after) = page.next_after else {
                break;
            };
            after = Some(next_after);
        }
        progress.validate()?;
        Ok(progress)
    }
}

async fn build_snapshot_in<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    schema_id: Uuid,
    entry_id: Uuid,
    source_locale: &str,
    target_locale: &str,
) -> FlexStandaloneTranslationResult<FlexStandaloneTranslationExactLocaleSnapshot> {
    let entry = flex_entries::Entity::find_by_id(entry_id)
        .filter(flex_entries::Column::TenantId.eq(tenant_id))
        .filter(flex_entries::Column::SchemaId.eq(schema_id))
        .one(db)
        .await
        .map_err(database_error)?
        .ok_or(FlexStandaloneTranslationError::EntryNotFound { schema_id, entry_id })?;
    let schema = flex_schemas::Entity::find_by_id(schema_id)
        .filter(flex_schemas::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(database_error)?
        .ok_or(FlexStandaloneTranslationError::EntryNotFound { schema_id, entry_id })?;
    let definitions = eligible_definitions(schema.fields_config.clone())?;
    if definitions.is_empty() {
        return Err(FlexStandaloneTranslationError::SourceLocaleNotFound {
            schema_id,
            entry_id,
            locale: source_locale.to_string(),
        });
    }
    let localized = flex_entry_localized_values::Entity::find()
        .filter(flex_entry_localized_values::Column::TenantId.eq(tenant_id))
        .filter(flex_entry_localized_values::Column::EntryId.eq(entry_id))
        .all(db)
        .await
        .map_err(database_error)?;
    let source_row = localized.iter().find(|row| row.locale == source_locale);
    let source_values = exact_values_for_locale(
        &definitions,
        source_row.map(|row| &row.data),
        source_locale,
    )?;
    if source_values.is_empty() {
        return Err(FlexStandaloneTranslationError::SourceLocaleNotFound {
            schema_id,
            entry_id,
            locale: source_locale.to_string(),
        });
    }
    let target_row = localized.iter().find(|row| row.locale == target_locale);
    let target_values = exact_values_for_locale(
        &definitions,
        target_row.map(|row| &row.data),
        target_locale,
    )?;
    let revision = RevisionRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT revision FROM flex_standalone_translation_resource_state WHERE tenant_id = $1 AND entry_id = $2",
        vec![tenant_id.into(), entry_id.into()],
    ))
    .one(db)
    .await
    .map_err(database_error)?
    .ok_or_else(|| {
        FlexStandaloneTranslationError::OwnerInvariant(format!(
            "standalone Translation resource {entry_id} has no durable revision state"
        ))
    })?;
    if revision.revision <= 0 {
        return Err(FlexStandaloneTranslationError::OwnerInvariant(format!(
            "standalone Translation resource {entry_id} has non-positive revision"
        )));
    }

    let required = definitions
        .iter()
        .map(|definition| (definition.field_key.as_str(), definition.is_required))
        .collect::<BTreeMap<_, _>>();
    let leaves = source_values
        .iter()
        .map(|(leaf, source_value)| FlexStandaloneTranslationLeafSnapshot {
            leaf: leaf.clone(),
            required: *required.get(leaf.field_key.as_str()).unwrap_or(&false),
            source_value: source_value.clone(),
            target_value: target_values.get(leaf).cloned(),
        })
        .collect::<Vec<_>>();
    let mut exact_locales = Vec::new();
    for row in &localized {
        if !exact_values_for_locale(&definitions, Some(&row.data), &row.locale)?.is_empty() {
            exact_locales.push(row.locale.clone());
        }
    }
    exact_locales.sort();
    exact_locales.dedup();

    let snapshot = FlexStandaloneTranslationExactLocaleSnapshot {
        schema_id,
        entry_id,
        is_active: schema.is_active,
        source_locale: source_locale.to_string(),
        target_locale: target_locale.to_string(),
        resource_revision: format!("standalone:{}", revision.revision),
        source_revision: locale_revision(
            tenant_id,
            schema_id,
            entry_id,
            source_locale,
            &source_values,
        ),
        target_revision: (!target_values.is_empty()).then(|| {
            locale_revision(
                tenant_id,
                schema_id,
                entry_id,
                target_locale,
                &target_values,
            )
        }),
        exact_locales,
        leaves,
    };
    let _ = entry;
    snapshot.validate()?;
    Ok(snapshot)
}

fn eligible_definitions(
    fields_config: JsonValue,
) -> FlexStandaloneTranslationResult<Vec<FieldDefinition>> {
    let mut definitions = parse_standalone_fields_config(fields_config)
        .map_err(|error| FlexStandaloneTranslationError::Storage(error.to_string()))?
        .into_iter()
        .filter(flex_standalone_translation_field_eligible)
        .collect::<Vec<_>>();
    definitions.sort_by(|left, right| left.field_key.cmp(&right.field_key));
    Ok(definitions)
}

fn exact_values_for_locale(
    definitions: &[FieldDefinition],
    values: Option<&JsonValue>,
    locale: &str,
) -> FlexStandaloneTranslationResult<BTreeMap<FlexStandaloneTranslationLeaf, String>> {
    let mut exact = BTreeMap::new();
    let Some(values) = values.and_then(JsonValue::as_object) else {
        return Ok(exact);
    };
    for definition in definitions {
        let Some(value) = values.get(&definition.field_key) else {
            continue;
        };
        match value {
            JsonValue::Null => {}
            JsonValue::String(value) if value.trim().is_empty() => {
                return Err(FlexStandaloneTranslationError::OwnerInvariant(format!(
                    "persisted standalone Translation value is blank for {locale}/{}",
                    definition.field_key
                )));
            }
            JsonValue::String(value) => {
                exact.insert(
                    FlexStandaloneTranslationLeaf {
                        field_key: definition.field_key.clone(),
                    },
                    value.clone(),
                );
            }
            _ => {
                return Err(FlexStandaloneTranslationError::OwnerInvariant(format!(
                    "persisted standalone Translation value is not text for {locale}/{}",
                    definition.field_key
                )));
            }
        }
    }
    Ok(exact)
}

fn validate_requested_targets(
    definitions: &[FieldDefinition],
    requested: &BTreeMap<FlexStandaloneTranslationLeaf, Option<String>>,
) -> FlexStandaloneTranslationResult<()> {
    let by_key = definitions
        .iter()
        .map(|definition| (definition.field_key.as_str(), definition))
        .collect::<BTreeMap<_, _>>();
    for (leaf, value) in requested {
        let definition = by_key.get(leaf.field_key.as_str()).ok_or_else(|| {
            FlexStandaloneTranslationError::Invalid(format!(
                "Flex standalone translation field {} is no longer eligible",
                leaf.field_key
            ))
        })?;
        match value {
            None if definition.is_required => {
                return Err(FlexStandaloneTranslationError::Invalid(format!(
                    "required standalone Translation field {} cannot be removed",
                    leaf.field_key
                )));
            }
            None => {}
            Some(value) => {
                let mut object = Map::new();
                object.insert(leaf.field_key.clone(), JsonValue::String(value.clone()));
                let validation_schema = CustomFieldsSchema::new(vec![(*definition).clone()]);
                let errors = validation_schema.validate(&JsonValue::Object(object));
                if !errors.is_empty() {
                    return Err(FlexStandaloneTranslationError::Invalid(format!(
                        "translated standalone field {} violates the current Flex definition: {errors:?}",
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
    schema_id: Uuid,
    entry_id: Uuid,
    locale: &str,
    values: &BTreeMap<FlexStandaloneTranslationLeaf, String>,
) -> String {
    let mut hasher = Sha256::new();
    hash_str(&mut hasher, LOCALE_REVISION_NAMESPACE);
    hasher.update(tenant_id.as_bytes());
    hasher.update(schema_id.as_bytes());
    hasher.update(entry_id.as_bytes());
    hash_str(&mut hasher, locale);
    hasher.update((values.len() as u64).to_be_bytes());
    for (leaf, value) in values {
        hash_str(&mut hasher, &leaf.field_key);
        hash_str(&mut hasher, value);
    }
    format!("flex-standalone-locale-v1:{}", hex::encode(hasher.finalize()))
}

fn hash_str(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn ensure_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> FlexStandaloneTranslationResult<()> {
    if expected != current {
        return Err(FlexStandaloneTranslationError::RevisionConflict { revision });
    }
    Ok(())
}

fn verify_applied_targets(
    requested: &BTreeMap<FlexStandaloneTranslationLeaf, Option<String>>,
    snapshot: &FlexStandaloneTranslationExactLocaleSnapshot,
) -> FlexStandaloneTranslationResult<()> {
    let actual = snapshot
        .leaves
        .iter()
        .map(|leaf| (leaf.leaf.clone(), leaf.target_value.clone()))
        .collect::<BTreeMap<_, _>>();
    if &actual != requested {
        return Err(FlexStandaloneTranslationError::OwnerInvariant(
            "standalone exact target state does not match the applied target set".to_string(),
        ));
    }
    Ok(())
}

fn observe_snapshot(
    progress: &mut FlexStandaloneTranslationExactProgress,
    snapshot: &FlexStandaloneTranslationExactLocaleSnapshot,
) -> FlexStandaloneTranslationResult<()> {
    checked_increment(&mut progress.resources, "resources")?;
    let mut complete = true;
    for leaf in &snapshot.leaves {
        let exact = leaf.target_value.is_some();
        if leaf.required {
            checked_increment(&mut progress.required_units, "required_units")?;
            if exact {
                checked_increment(&mut progress.exact_required_units, "exact_required_units")?;
            } else {
                complete = false;
            }
        } else {
            checked_increment(&mut progress.optional_units, "optional_units")?;
            if exact {
                checked_increment(&mut progress.exact_optional_units, "exact_optional_units")?;
            }
        }
    }
    if complete {
        checked_increment(&mut progress.complete_resources, "complete_resources")?;
    }
    Ok(())
}

fn checked_increment(
    value: &mut u64,
    label: &str,
) -> FlexStandaloneTranslationResult<()> {
    *value = value.checked_add(1).ok_or_else(|| {
        FlexStandaloneTranslationError::OwnerInvariant(format!(
            "standalone Translation progress `{label}` overflowed u64"
        ))
    })?;
    Ok(())
}

fn decode_receipt(
    value: JsonValue,
    expected_schema_id: Uuid,
    expected_entry_id: Uuid,
) -> FlexStandaloneTranslationResult<FlexStandaloneTranslationExactLocaleApplyReceipt> {
    let receipt: FlexStandaloneTranslationExactLocaleApplyReceipt = serde_json::from_value(value)
        .map_err(|error| {
            FlexStandaloneTranslationError::Operation(PortError::invariant_violation(
                "outbox.operation_receipt_corrupt",
                error.to_string(),
            ))
        })?;
    if receipt.schema_id != expected_schema_id
        || receipt.entry_id != expected_entry_id
        || receipt.operation_id.is_nil()
        || receipt.resource_revision.trim().is_empty()
        || receipt.target_revision.trim().is_empty()
        || receipt.target_values.is_empty()
    {
        return Err(FlexStandaloneTranslationError::Operation(
            PortError::invariant_violation(
                "outbox.operation_receipt_corrupt",
                "standalone Translation owner receipt violates its identity contract",
            ),
        ));
    }
    Ok(receipt)
}

fn validate_uuid(value: Uuid, label: &str) -> FlexStandaloneTranslationResult<()> {
    if value.is_nil() {
        return Err(FlexStandaloneTranslationError::Invalid(format!(
            "Flex standalone translation {label} must not be the nil UUID"
        )));
    }
    Ok(())
}

fn ensure_postgres(db: &DatabaseConnection) -> FlexStandaloneTranslationResult<()> {
    if db.get_database_backend() != DatabaseBackend::Postgres {
        return Err(FlexStandaloneTranslationError::Invalid(
            "Flex standalone Translation requires PostgreSQL durable revision state".to_string(),
        ));
    }
    Ok(())
}

fn owner_error_to_port_error(error: &FlexStandaloneTranslationError) -> PortError {
    match error {
        FlexStandaloneTranslationError::Invalid(_) => PortError::validation(
            "flex.standalone_translation_owner_validation",
            "Flex rejected the standalone translation request",
        ),
        FlexStandaloneTranslationError::EntryNotFound { .. } => PortError::not_found(
            "flex.standalone_translation_resource_not_found",
            "Standalone Flex entry was not found",
        ),
        FlexStandaloneTranslationError::SourceLocaleNotFound { .. } => PortError::not_found(
            "flex.standalone_translation_source_not_found",
            "Exact source standalone Flex locale was not found",
        ),
        FlexStandaloneTranslationError::RevisionConflict { .. } => PortError::conflict(
            "flex.standalone_translation_revision_conflict",
            "Standalone Flex translation state conflicts with the request",
        ),
        FlexStandaloneTranslationError::Operation(error) => error.clone(),
        FlexStandaloneTranslationError::Storage(_) => PortError::unavailable(
            "flex.standalone_translation_owner_unavailable",
            "Standalone Flex translation storage is temporarily unavailable",
        ),
        FlexStandaloneTranslationError::OwnerInvariant(_) => PortError::invariant_violation(
            "flex.standalone_translation_owner_invariant",
            "Standalone Flex translation owner state is invalid",
        ),
    }
}

fn database_error(error: sea_orm::DbErr) -> FlexStandaloneTranslationError {
    FlexStandaloneTranslationError::Storage(error.to_string())
}
