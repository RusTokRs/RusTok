use std::collections::{BTreeMap, BTreeSet, HashMap};

use async_trait::async_trait;
use flex::{
    FlexSchemaTranslationError, FlexSchemaTranslationExactLocaleApply,
    FlexSchemaTranslationExactLocaleApplyReceipt, FlexSchemaTranslationExactLocaleSnapshot,
    FlexSchemaTranslationLeaf, FlexSchemaTranslationLeafSnapshot, FlexSchemaTranslationOwnerPort,
    FlexSchemaTranslationResourcePage, FlexSchemaTranslationResult,
    FlexSchemaTranslationTargetValue, UpdateFlexSchemaCommand,
    apply_schema_definition_translation_targets, parse_standalone_fields_config,
    schema_definition_translation_exact_values, schema_definition_translation_locales,
    serialize_standalone_fields_config, validate_flex_schema_translation_locale_pair,
    validate_flex_schema_translation_resource_page, validate_update_schema_command,
};
use rustok_api::{PortError, normalize_locale_tag};
use rustok_events::DomainEvent;
use rustok_outbox::{TransactionalEventBus, idempotency};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::models::{flex_schema_translations, flex_schemas};

const OWNER_SLUG: &str = "flex";
const RESOURCE_KIND: &str = "schema_copy";
const OPERATION_APPLY_PATCH: &str = "translation_target_apply_schema_copy_patch";
const RESOURCE_REVISION_NAMESPACE: &str = "rustok-flex/schema-copy-resource/v1";
const LOCALE_REVISION_NAMESPACE: &str = "rustok-flex/schema-copy-locale/v1";
const LEGACY_UNDETERMINED_LOCALE: &str = "und";

#[derive(Clone)]
pub struct ServerFlexSchemaTranslationOwner {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ServerFlexSchemaTranslationOwner {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    async fn apply_under_lease(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        schema_id: Uuid,
        request: FlexSchemaTranslationExactLocaleApply,
        lease: idempotency::Lease,
    ) -> FlexSchemaTranslationResult<FlexSchemaTranslationExactLocaleApplyReceipt> {
        let txn = self.db.begin().await.map_err(database_error)?;

        // The language-agnostic schema parent is the serialization point for a schema-copy
        // apply. Existing exact locale rows are locked as well; an absent target row is
        // protected by the parent lock for provider writes and by the composite PK against
        // a concurrent legacy/direct insert.
        let schema = flex_schemas::Entity::find_by_id(schema_id)
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await
            .map_err(database_error)?
            .ok_or(FlexSchemaTranslationError::SchemaNotFound(schema_id))?;
        let translations = load_schema_translations_locked(&txn, schema_id).await?;

        let before = build_snapshot(
            &schema,
            &translations,
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
            return Err(FlexSchemaTranslationError::RevisionConflict {
                revision: "target",
            });
        }

        let requested_targets = request
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
        let requested_leaves = requested_targets.keys().cloned().collect::<BTreeSet<_>>();
        if source_leaves != requested_leaves {
            return Err(FlexSchemaTranslationError::Invalid(
                "Flex schema translation apply must contain exactly the source-visible leaf set"
                    .to_string(),
            ));
        }

        let target_name = required_requested_value(
            &requested_targets,
            &FlexSchemaTranslationLeaf::SchemaName,
        )?;
        let source_has_description =
            source_leaves.contains(&FlexSchemaTranslationLeaf::SchemaDescription);
        let requested_description = source_has_description
            .then(|| {
                requested_targets
                    .get(&FlexSchemaTranslationLeaf::SchemaDescription)
                    .cloned()
                    .flatten()
            })
            .flatten();

        let mut definitions = parse_standalone_fields_config(schema.fields_config.clone())
            .map_err(|error| persisted_contract_error("fields_config", error))?;
        let definition_targets = requested_targets
            .iter()
            .filter(|(leaf, _)| {
                !matches!(
                    leaf,
                    FlexSchemaTranslationLeaf::SchemaName
                        | FlexSchemaTranslationLeaf::SchemaDescription
                )
            })
            .map(|(leaf, value)| (leaf.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        let fields_changed = apply_schema_definition_translation_targets(
            &mut definitions,
            &request.source_locale,
            &request.target_locale,
            &definition_targets,
        )?;
        validate_update_schema_command(&UpdateFlexSchemaCommand {
            fields_config: Some(definitions.clone()),
            ..Default::default()
        })
        .map_err(|error| {
            FlexSchemaTranslationError::Invalid(format!(
                "Flex rejected translated schema field definitions: {error}"
            ))
        })?;

        let schema_after_fields = if fields_changed {
            let mut active: flex_schemas::ActiveModel = schema.clone().into();
            active.fields_config = Set(
                serialize_standalone_fields_config(definitions)
                    .map_err(|error| persisted_contract_error("fields_config", error))?,
            );
            active.update(&txn).await.map_err(database_error)?
        } else {
            schema.clone()
        };

        let existing_target = translations
            .iter()
            .find(|translation| translation.locale == request.target_locale);
        let target_description = if source_has_description {
            requested_description
        } else {
            existing_target.and_then(|translation| translation.description.clone())
        };
        validate_schema_name(&target_name)?;
        if let Some(description) = &target_description {
            validate_schema_description(description)?;
        }

        let row_changed = match existing_target {
            Some(existing) => {
                let changed = existing.name != target_name
                    || existing.description != target_description;
                if changed {
                    let mut active: flex_schema_translations::ActiveModel = existing.clone().into();
                    active.name = Set(target_name.clone());
                    active.description = Set(target_description.clone());
                    active.update(&txn).await.map_err(database_error)?;
                }
                changed
            }
            None => {
                flex_schema_translations::ActiveModel {
                    schema_id: Set(schema_id),
                    locale: Set(request.target_locale.clone()),
                    name: Set(target_name.clone()),
                    description: Set(target_description.clone()),
                    created_at: sea_orm::ActiveValue::NotSet,
                    updated_at: sea_orm::ActiveValue::NotSet,
                }
                .insert(&txn)
                .await
                .map_err(database_error)?;
                true
            }
        };

        let translations_after = load_schema_translations(&txn, schema_id).await?;
        let after = build_snapshot(
            &schema_after_fields,
            &translations_after,
            &request.source_locale,
            &request.target_locale,
        )?;
        let target_revision = after.target_revision.clone().ok_or_else(|| {
            FlexSchemaTranslationError::OwnerInvariant(
                "exact target locale is absent after Flex schema translation apply".to_string(),
            )
        })?;
        verify_applied_targets(&requested_targets, &after)?;

        let changed = fields_changed || row_changed;
        if changed {
            self.event_bus
                .publish_in_tx_with_envelope_id(
                    &txn,
                    tenant_id,
                    actor_user_id,
                    DomainEvent::TranslationTargetChanged {
                        owner_slug: OWNER_SLUG.to_string(),
                        resource_kind: RESOURCE_KIND.to_string(),
                        resource_id: schema_id.to_string(),
                        changed_locale: request.target_locale.clone(),
                        resource_revision: after.resource_revision.clone(),
                        target_revision: target_revision.clone(),
                        operation: "apply_exact_locale".to_string(),
                        correlation_id: lease.operation_id.to_string(),
                    },
                )
                .await
                .map_err(|error| {
                    FlexSchemaTranslationError::OwnerInvariant(format!(
                        "failed to publish Flex schema translation owner event: {error}"
                    ))
                })?;
        }

        let target_values = requested_targets
            .into_iter()
            .map(|(leaf, value)| FlexSchemaTranslationTargetValue { leaf, value })
            .collect();
        let receipt = FlexSchemaTranslationExactLocaleApplyReceipt {
            operation_id: lease.operation_id,
            schema_id,
            resource_revision: after.resource_revision,
            target_revision,
            target_values,
        };
        idempotency::complete(&txn, lease, &receipt)
            .await
            .map_err(FlexSchemaTranslationError::Operation)?;
        txn.commit().await.map_err(database_error)?;
        Ok(receipt)
    }

    async fn fail_receipt(&self, lease: idempotency::Lease, error: &FlexSchemaTranslationError) {
        let port_error = owner_error_to_port_error(error);
        if let Err(receipt_error) = idempotency::fail(&self.db, lease, &port_error).await {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "Failed to persist Flex schema translation owner failure receipt"
            );
        }
    }
}

#[async_trait]
impl FlexSchemaTranslationOwnerPort for ServerFlexSchemaTranslationOwner {
    async fn list_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<Uuid>,
        limit: u16,
    ) -> FlexSchemaTranslationResult<FlexSchemaTranslationResourcePage> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_flex_schema_translation_locale_pair(source_locale, target_locale)?;
        validate_flex_schema_translation_resource_page(limit)?;
        if let Some(after) = after {
            validate_uuid(after, "after")?;
        }

        // Schema name is the required anchor of schema-copy identity. A resource is source
        // eligible only when that exact source-locale row exists; field maps never invent
        // a missing source schema name through fallback.
        let source_schema_ids = sea_orm::sea_query::Query::select()
            .column(flex_schema_translations::Column::SchemaId)
            .from(flex_schema_translations::Entity)
            .and_where(
                sea_orm::sea_query::Expr::col(flex_schema_translations::Column::Locale)
                    .eq(source_locale.to_string()),
            )
            .to_owned();
        let mut query = flex_schemas::Entity::find()
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .filter(flex_schemas::Column::Id.in_subquery(source_schema_ids))
            .order_by_asc(flex_schemas::Column::Id);
        if let Some(after) = after {
            query = query.filter(flex_schemas::Column::Id.gt(after));
        }

        let mut schemas = query
            .limit(u64::from(limit) + 1)
            .all(&self.db)
            .await
            .map_err(database_error)?;
        let has_more = schemas.len() > usize::from(limit);
        if has_more {
            schemas.truncate(usize::from(limit));
        }
        let next_after = has_more.then(|| schemas.last().map(|schema| schema.id)).flatten();
        if schemas.is_empty() {
            return Ok(FlexSchemaTranslationResourcePage {
                resources: Vec::new(),
                next_after: None,
            });
        }

        let schema_ids = schemas.iter().map(|schema| schema.id).collect::<Vec<_>>();
        let grouped = load_schema_translation_map(&self.db, &schema_ids).await?;
        let mut resources = Vec::with_capacity(schemas.len());
        for schema in schemas {
            let translations = grouped.get(&schema.id).cloned().unwrap_or_default();
            resources.push(build_snapshot(
                &schema,
                &translations,
                source_locale,
                target_locale,
            )?);
        }
        Ok(FlexSchemaTranslationResourcePage {
            resources,
            next_after,
        })
    }

    async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexSchemaTranslationResult<FlexSchemaTranslationExactLocaleSnapshot> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_uuid(schema_id, "schema_id")?;
        validate_flex_schema_translation_locale_pair(source_locale, target_locale)?;

        let schema = flex_schemas::Entity::find_by_id(schema_id)
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(database_error)?
            .ok_or(FlexSchemaTranslationError::SchemaNotFound(schema_id))?;
        let translations = load_schema_translations(&self.db, schema_id).await?;
        build_snapshot(&schema, &translations, source_locale, target_locale)
    }

    async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        schema_id: Uuid,
        request: FlexSchemaTranslationExactLocaleApply,
    ) -> FlexSchemaTranslationResult<FlexSchemaTranslationExactLocaleApplyReceipt> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_uuid(schema_id, "schema_id")?;
        if let Some(actor_user_id) = actor_user_id {
            validate_uuid(actor_user_id, "actor_user_id")?;
        }
        request.validate()?;

        let lease = match idempotency::admit(
            &self.db,
            idempotency::OwnerOperationScope::Tenant(tenant_id),
            OWNER_SLUG,
            &request.operation.idempotency_key,
            OPERATION_APPLY_PATCH,
            &request,
        )
        .await
        .map_err(FlexSchemaTranslationError::Operation)?
        {
            idempotency::Admission::Run(lease) => lease,
            idempotency::Admission::Replay(value) => return decode_receipt(value),
            idempotency::Admission::ReplayError(error) => {
                return Err(FlexSchemaTranslationError::Operation(error));
            }
        };

        let result = self
            .apply_under_lease(tenant_id, actor_user_id, schema_id, request, lease)
            .await;
        if let Err(error) = &result {
            self.fail_receipt(lease, error).await;
        }
        result
    }
}

fn build_snapshot(
    schema: &flex_schemas::Model,
    translations: &[flex_schema_translations::Model],
    source_locale: &str,
    target_locale: &str,
) -> FlexSchemaTranslationResult<FlexSchemaTranslationExactLocaleSnapshot> {
    validate_flex_schema_translation_locale_pair(source_locale, target_locale)?;
    let definitions = parse_standalone_fields_config(schema.fields_config.clone())
        .map_err(|error| persisted_contract_error("fields_config", error))?;
    let source_row = exact_translation(translations, source_locale).ok_or_else(|| {
        FlexSchemaTranslationError::SourceLocaleNotFound {
            schema_id: schema.id,
            locale: source_locale.to_string(),
        }
    })?;
    validate_schema_translation_row(source_row)?;
    if let Some(target_row) = exact_translation(translations, target_locale) {
        validate_schema_translation_row(target_row)?;
    }

    let source_values = exact_locale_values(&definitions, Some(source_row), source_locale)?;
    let target_row = exact_translation(translations, target_locale);
    let target_values = exact_locale_values(&definitions, target_row, target_locale)?;
    let leaves = source_values
        .iter()
        .map(|(leaf, source_value)| FlexSchemaTranslationLeafSnapshot {
            leaf: leaf.clone(),
            source_value: source_value.clone(),
            target_value: target_values.get(leaf).cloned(),
        })
        .collect::<Vec<_>>();

    let mut exact_locales = schema_definition_translation_locales(&definitions)?;
    for translation in translations {
        if translation.locale == LEGACY_UNDETERMINED_LOCALE {
            continue;
        }
        validate_runtime_locale(&translation.locale)?;
        validate_schema_translation_row(translation)?;
        exact_locales.insert(translation.locale.clone());
    }

    Ok(FlexSchemaTranslationExactLocaleSnapshot {
        schema_id: schema.id,
        slug: schema.slug.clone(),
        is_active: schema.is_active,
        source_locale: source_locale.to_string(),
        target_locale: target_locale.to_string(),
        resource_revision: resource_revision(schema, translations),
        source_revision: locale_revision(schema.id, source_locale, &source_values),
        target_revision: (!target_values.is_empty())
            .then(|| locale_revision(schema.id, target_locale, &target_values)),
        exact_locales: exact_locales.into_iter().collect(),
        leaves,
    })
}

fn exact_locale_values(
    definitions: &[rustok_core::field_schema::FieldDefinition],
    row: Option<&flex_schema_translations::Model>,
    locale: &str,
) -> FlexSchemaTranslationResult<BTreeMap<FlexSchemaTranslationLeaf, String>> {
    let mut values = schema_definition_translation_exact_values(definitions, locale)?;
    if let Some(row) = row {
        validate_schema_translation_row(row)?;
        values.insert(FlexSchemaTranslationLeaf::SchemaName, row.name.clone());
        if let Some(description) = &row.description {
            values.insert(
                FlexSchemaTranslationLeaf::SchemaDescription,
                description.clone(),
            );
        }
    }
    Ok(values)
}

fn exact_translation<'a>(
    translations: &'a [flex_schema_translations::Model],
    locale: &str,
) -> Option<&'a flex_schema_translations::Model> {
    translations
        .iter()
        .find(|translation| translation.locale == locale)
}

async fn load_schema_translations<C>(
    db: &C,
    schema_id: Uuid,
) -> FlexSchemaTranslationResult<Vec<flex_schema_translations::Model>>
where
    C: ConnectionTrait,
{
    flex_schema_translations::Entity::find()
        .filter(flex_schema_translations::Column::SchemaId.eq(schema_id))
        .order_by_asc(flex_schema_translations::Column::Locale)
        .all(db)
        .await
        .map_err(database_error)
}

async fn load_schema_translations_locked<C>(
    db: &C,
    schema_id: Uuid,
) -> FlexSchemaTranslationResult<Vec<flex_schema_translations::Model>>
where
    C: ConnectionTrait,
{
    flex_schema_translations::Entity::find()
        .filter(flex_schema_translations::Column::SchemaId.eq(schema_id))
        .order_by_asc(flex_schema_translations::Column::Locale)
        .lock_exclusive()
        .all(db)
        .await
        .map_err(database_error)
}

async fn load_schema_translation_map<C>(
    db: &C,
    schema_ids: &[Uuid],
) -> FlexSchemaTranslationResult<HashMap<Uuid, Vec<flex_schema_translations::Model>>>
where
    C: ConnectionTrait,
{
    if schema_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = flex_schema_translations::Entity::find()
        .filter(flex_schema_translations::Column::SchemaId.is_in(schema_ids.to_vec()))
        .order_by_asc(flex_schema_translations::Column::SchemaId)
        .order_by_asc(flex_schema_translations::Column::Locale)
        .all(db)
        .await
        .map_err(database_error)?;
    let mut grouped = HashMap::<Uuid, Vec<flex_schema_translations::Model>>::new();
    for row in rows {
        grouped.entry(row.schema_id).or_default().push(row);
    }
    Ok(grouped)
}

fn resource_revision(
    schema: &flex_schemas::Model,
    translations: &[flex_schema_translations::Model],
) -> String {
    let mut hasher = Sha256::new();
    hash_str(&mut hasher, RESOURCE_REVISION_NAMESPACE);
    hash_uuid(&mut hasher, schema.tenant_id);
    hash_uuid(&mut hasher, schema.id);
    hash_str(&mut hasher, &schema.slug);
    hash_bool(&mut hasher, schema.is_active);
    hash_json(&mut hasher, &schema.fields_config);

    let mut ordered = translations.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.locale.cmp(&right.locale));
    hash_len(&mut hasher, ordered.len());
    for translation in ordered {
        hash_str(&mut hasher, &translation.locale);
        hash_str(&mut hasher, &translation.name);
        hash_optional_str(&mut hasher, translation.description.as_deref());
    }
    format!("flex-schema-resource-v1:{}", hex::encode(hasher.finalize()))
}

fn locale_revision(
    schema_id: Uuid,
    locale: &str,
    values: &BTreeMap<FlexSchemaTranslationLeaf, String>,
) -> String {
    let mut hasher = Sha256::new();
    hash_str(&mut hasher, LOCALE_REVISION_NAMESPACE);
    hash_uuid(&mut hasher, schema_id);
    hash_str(&mut hasher, locale);
    hash_len(&mut hasher, values.len());
    for (leaf, value) in values {
        hash_leaf(&mut hasher, leaf);
        hash_str(&mut hasher, value);
    }
    format!("flex-schema-locale-v1:{}", hex::encode(hasher.finalize()))
}

fn hash_leaf(hasher: &mut Sha256, leaf: &FlexSchemaTranslationLeaf) {
    match leaf {
        FlexSchemaTranslationLeaf::SchemaName => hash_str(hasher, "schema_name"),
        FlexSchemaTranslationLeaf::SchemaDescription => hash_str(hasher, "schema_description"),
        FlexSchemaTranslationLeaf::FieldLabel { field_key } => {
            hash_str(hasher, "field_label");
            hash_str(hasher, field_key);
        }
        FlexSchemaTranslationLeaf::FieldDescription { field_key } => {
            hash_str(hasher, "field_description");
            hash_str(hasher, field_key);
        }
        FlexSchemaTranslationLeaf::FieldValidationErrorMessage { field_key } => {
            hash_str(hasher, "field_validation_error_message");
            hash_str(hasher, field_key);
        }
        FlexSchemaTranslationLeaf::FieldOptionLabel {
            field_key,
            option_value,
        } => {
            hash_str(hasher, "field_option_label");
            hash_str(hasher, field_key);
            hash_str(hasher, option_value);
        }
    }
}

fn hash_json(hasher: &mut Sha256, value: &JsonValue) {
    match value {
        JsonValue::Null => hash_str(hasher, "null"),
        JsonValue::Bool(value) => {
            hash_str(hasher, "bool");
            hash_bool(hasher, *value);
        }
        JsonValue::Number(value) => {
            hash_str(hasher, "number");
            hash_str(hasher, &value.to_string());
        }
        JsonValue::String(value) => {
            hash_str(hasher, "string");
            hash_str(hasher, value);
        }
        JsonValue::Array(values) => {
            hash_str(hasher, "array");
            hash_len(hasher, values.len());
            for value in values {
                hash_json(hasher, value);
            }
        }
        JsonValue::Object(values) => {
            hash_str(hasher, "object");
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort();
            hash_len(hasher, keys.len());
            for key in keys {
                hash_str(hasher, key);
                hash_json(hasher, &values[key]);
            }
        }
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

fn hash_bool(hasher: &mut Sha256, value: bool) {
    hasher.update([u8::from(value)]);
}

fn hash_len(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_be_bytes());
}

fn verify_applied_targets(
    requested: &BTreeMap<FlexSchemaTranslationLeaf, Option<String>>,
    snapshot: &FlexSchemaTranslationExactLocaleSnapshot,
) -> FlexSchemaTranslationResult<()> {
    let actual = snapshot
        .leaves
        .iter()
        .map(|leaf| (leaf.leaf.clone(), leaf.target_value.clone()))
        .collect::<BTreeMap<_, _>>();
    if &actual != requested {
        return Err(FlexSchemaTranslationError::OwnerInvariant(
            "Flex schema exact target state does not match the applied target set".to_string(),
        ));
    }
    Ok(())
}

fn required_requested_value(
    requested: &BTreeMap<FlexSchemaTranslationLeaf, Option<String>>,
    leaf: &FlexSchemaTranslationLeaf,
) -> FlexSchemaTranslationResult<String> {
    requested
        .get(leaf)
        .and_then(|value| value.clone())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            FlexSchemaTranslationError::Invalid(format!(
                "Flex schema translation required leaf {leaf:?} cannot be removed"
            ))
        })
}

fn validate_schema_translation_row(
    translation: &flex_schema_translations::Model,
) -> FlexSchemaTranslationResult<()> {
    if translation.locale != LEGACY_UNDETERMINED_LOCALE {
        validate_runtime_locale(&translation.locale)?;
    }
    validate_schema_name(&translation.name)?;
    if let Some(description) = &translation.description {
        validate_schema_description(description)?;
    }
    Ok(())
}

fn validate_schema_name(value: &str) -> FlexSchemaTranslationResult<()> {
    if value.trim().is_empty() || value.trim() != value || value.len() > 255 {
        return Err(FlexSchemaTranslationError::OwnerInvariant(
            "Flex schema translation name must be normalized, nonblank, and at most 255 bytes"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_schema_description(value: &str) -> FlexSchemaTranslationResult<()> {
    if value.trim().is_empty() || value.trim() != value {
        return Err(FlexSchemaTranslationError::OwnerInvariant(
            "Flex schema translation description must be normalized and nonblank when present"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_runtime_locale(locale: &str) -> FlexSchemaTranslationResult<()> {
    if locale == LEGACY_UNDETERMINED_LOCALE || normalize_locale_tag(locale).as_deref() != Some(locale)
    {
        return Err(FlexSchemaTranslationError::OwnerInvariant(format!(
            "Flex schema translation row contains invalid runtime locale `{locale}`"
        )));
    }
    Ok(())
}

fn ensure_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> FlexSchemaTranslationResult<()> {
    if expected != current {
        return Err(FlexSchemaTranslationError::RevisionConflict { revision });
    }
    Ok(())
}

fn validate_uuid(value: Uuid, label: &str) -> FlexSchemaTranslationResult<()> {
    if value.is_nil() {
        return Err(FlexSchemaTranslationError::Invalid(format!(
            "Flex schema translation {label} must not be the nil UUID"
        )));
    }
    Ok(())
}

fn decode_receipt(
    value: JsonValue,
) -> FlexSchemaTranslationResult<FlexSchemaTranslationExactLocaleApplyReceipt> {
    serde_json::from_value(value).map_err(|error| {
        FlexSchemaTranslationError::Operation(PortError::invariant_violation(
            "outbox.operation_receipt_corrupt",
            error.to_string(),
        ))
    })
}

fn persisted_contract_error(
    label: &str,
    error: rustok_core::field_schema::FlexError,
) -> FlexSchemaTranslationError {
    FlexSchemaTranslationError::OwnerInvariant(format!(
        "persisted Flex schema {label} violates the owner contract: {error}"
    ))
}

fn database_error(error: sea_orm::DbErr) -> FlexSchemaTranslationError {
    FlexSchemaTranslationError::Database(error.to_string())
}

fn owner_error_to_port_error(error: &FlexSchemaTranslationError) -> PortError {
    match error {
        FlexSchemaTranslationError::Invalid(_) => PortError::validation(
            "flex.schema_translation_owner_validation",
            "Flex rejected the schema translation request",
        ),
        FlexSchemaTranslationError::SchemaNotFound(_) => PortError::not_found(
            "flex.schema_translation_resource_not_found",
            "Flex schema translation resource was not found",
        ),
        FlexSchemaTranslationError::SourceLocaleNotFound { .. } => PortError::not_found(
            "flex.schema_translation_source_not_found",
            "Exact source Flex schema locale was not found",
        ),
        FlexSchemaTranslationError::RevisionConflict { .. } => PortError::conflict(
            "flex.schema_translation_revision_conflict",
            "Flex schema translation state conflicts with the request",
        ),
        FlexSchemaTranslationError::Operation(error) => error.clone(),
        FlexSchemaTranslationError::Database(_) => PortError::unavailable(
            "flex.schema_translation_owner_unavailable",
            "Flex schema translation storage is temporarily unavailable",
        ),
        FlexSchemaTranslationError::OwnerInvariant(_) => PortError::invariant_violation(
            "flex.schema_translation_owner_invariant",
            "Flex schema translation owner state is invalid",
        ),
    }
}
