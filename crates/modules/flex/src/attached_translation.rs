use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use async_trait::async_trait;
use rustok_api::normalize_locale_tag;
use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FieldType};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::attached::{Column, Entity};
use crate::{
    AttachedEntityRef, is_valid_flex_entity_type, load_exact_locale_values,
    persist_localized_values,
};

const RESOURCE_REVISION_NAMESPACE: &str = "rustok-flex/attached-localized-value-resource/v1";
const LOCALE_REVISION_NAMESPACE: &str = "rustok-flex/attached-localized-value-locale/v1";
const LEGACY_UNDETERMINED_LOCALE: &str = "und";

/// Translation-visible scalar kinds for attached Flex values.
///
/// This is deliberately narrower than `FieldType`: select values are machine keys,
/// URL/email/phone/date/number/bool/color values are not prose, and `Json` must never
/// become an arbitrary Translation payload channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlexAttachedTranslationScalarKind {
    Text,
    Textarea,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedTranslationFieldSnapshot {
    pub field_key: String,
    pub scalar_kind: FlexAttachedTranslationScalarKind,
    pub required: bool,
    pub source_value: String,
    /// Blank exact target rows are treated as untranslated while their physical
    /// presence still participates in target/resource revision computation.
    pub target_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedTranslationExactLocaleSnapshot {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    /// Only active, `is_localized` Text/Textarea fields with exact nonblank source
    /// values are exposed. No payload path is inferred from stored JSON.
    pub fields: Vec<FlexAttachedTranslationFieldSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedTranslationTargetValue {
    pub field_key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlexAttachedTranslationExactLocaleApply {
    pub source_locale: String,
    pub target_locale: String,
    /// Complete translated values for the exact source-visible field set returned by
    /// the owner snapshot. Translation does not expose deletion/null semantics here.
    pub target_values: Vec<FlexAttachedTranslationTargetValue>,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

impl FlexAttachedTranslationExactLocaleApply {
    pub fn validate(&self) -> FlexAttachedTranslationResult<()> {
        validate_flex_attached_translation_locale_pair(&self.source_locale, &self.target_locale)?;
        validate_nonblank(&self.expected_resource_revision, "expected_resource_revision")?;
        validate_nonblank(&self.expected_source_revision, "expected_source_revision")?;
        if let Some(revision) = &self.expected_target_revision {
            validate_nonblank(revision, "expected_target_revision")?;
        }
        if self.target_values.is_empty() {
            return Err(FlexAttachedTranslationError::Invalid(
                "Flex attached translation apply must contain at least one target value"
                    .to_string(),
            ));
        }

        let mut keys = BTreeSet::new();
        for target in &self.target_values {
            validate_identity_component(&target.field_key, "field_key")?;
            if !keys.insert(target.field_key.as_str()) {
                return Err(FlexAttachedTranslationError::Invalid(
                    "Flex attached translation apply contains a duplicate field key".to_string(),
                ));
            }
            if target.value.trim().is_empty() {
                return Err(FlexAttachedTranslationError::Invalid(
                    "Flex attached translation target values must be nonblank".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedTranslationExactLocaleApplyReceipt {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target_values: Vec<FlexAttachedTranslationTargetValue>,
}

#[derive(Debug)]
pub enum FlexAttachedTranslationError {
    Invalid(String),
    OwnerNotFound {
        entity_type: String,
        entity_id: Uuid,
    },
    SourceLocaleNotFound {
        entity_type: String,
        entity_id: Uuid,
        locale: String,
    },
    RevisionConflict {
        revision: &'static str,
    },
    Database(String),
    OwnerInvariant(String),
}

impl fmt::Display for FlexAttachedTranslationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::OwnerNotFound {
                entity_type,
                entity_id,
            } => write!(formatter, "Flex attached owner not found: {entity_type}/{entity_id}"),
            Self::SourceLocaleNotFound {
                entity_type,
                entity_id,
                locale,
            } => write!(
                formatter,
                "Flex attached exact source locale not found: {locale} for {entity_type}/{entity_id}"
            ),
            Self::RevisionConflict { revision } => {
                write!(formatter, "Flex attached translation {revision} revision conflict")
            }
            Self::Database(message) => {
                write!(formatter, "Flex attached translation database error: {message}")
            }
            Self::OwnerInvariant(message) => {
                write!(formatter, "Flex attached translation owner invariant failed: {message}")
            }
        }
    }
}

impl std::error::Error for FlexAttachedTranslationError {}

pub type FlexAttachedTranslationResult<T> = Result<T, FlexAttachedTranslationError>;

/// Exact read/apply owner boundary for registered reusable attached donors.
///
/// Donor identity/lifecycle validation and write serialization remain host concerns.
/// The core helper contract only touches Flex-owned field definitions and exact localized
/// rows; it never reads a donor table or presentation fallback.
#[async_trait]
pub trait FlexAttachedTranslationOwnerPort: Send + Sync {
    async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleSnapshot>;

    async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        request: FlexAttachedTranslationExactLocaleApply,
    ) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleApplyReceipt>;
}

pub fn validate_flex_attached_translation_locale_pair(
    source_locale: &str,
    target_locale: &str,
) -> FlexAttachedTranslationResult<()> {
    validate_authoring_locale(source_locale, "source_locale")?;
    validate_authoring_locale(target_locale, "target_locale")?;
    if source_locale == target_locale {
        return Err(FlexAttachedTranslationError::Invalid(
            "Flex attached translation source and target locale must differ".to_string(),
        ));
    }
    Ok(())
}

/// Build an exact source/target snapshot from Flex-owned storage only.
/// The caller must first prove that the donor identity exists in its canonical owner.
pub async fn read_flex_attached_translation_exact<C>(
    db: &C,
    entity: AttachedEntityRef<'_>,
    schema: &CustomFieldsSchema,
    source_locale: &str,
    target_locale: &str,
) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleSnapshot>
where
    C: ConnectionTrait,
{
    validate_entity(&entity)?;
    validate_flex_attached_translation_locale_pair(source_locale, target_locale)?;

    let definitions = translatable_definitions(schema);
    if definitions.is_empty() {
        return Err(FlexAttachedTranslationError::Invalid(format!(
            "Flex attached owner {} has no active localized Text/Textarea fields",
            entity.entity_type
        )));
    }

    let rows = Entity::find()
        .filter(Column::TenantId.eq(entity.tenant_id))
        .filter(Column::EntityType.eq(entity.entity_type))
        .filter(Column::EntityId.eq(entity.entity_id))
        .order_by_asc(Column::Locale)
        .order_by_asc(Column::FieldKey)
        .all(db)
        .await
        .map_err(database_error)?;

    build_snapshot(entity, &definitions, &rows, source_locale, target_locale)
}

/// Apply one exact target locale under a caller-owned transaction/serialization point.
///
/// This function performs all CAS checks again from the transaction-visible state and
/// mutates only the source-visible, schema-declared localized Text/Textarea keys. Other
/// exact attached keys are preserved byte-for-byte in the target locale.
pub async fn apply_flex_attached_translation_exact<C>(
    db: &C,
    entity: AttachedEntityRef<'_>,
    schema: &CustomFieldsSchema,
    request: &FlexAttachedTranslationExactLocaleApply,
) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleApplyReceipt>
where
    C: ConnectionTrait,
{
    validate_entity(&entity)?;
    request.validate()?;

    let before = read_flex_attached_translation_exact(
        db,
        entity.clone(),
        schema,
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
        return Err(FlexAttachedTranslationError::RevisionConflict {
            revision: "target",
        });
    }

    let source_keys = before
        .fields
        .iter()
        .map(|field| field.field_key.as_str())
        .collect::<BTreeSet<_>>();
    let requested_keys = request
        .target_values
        .iter()
        .map(|target| target.field_key.as_str())
        .collect::<BTreeSet<_>>();
    if source_keys != requested_keys {
        return Err(FlexAttachedTranslationError::Invalid(
            "Flex attached translation apply must contain exactly the source-visible field set"
                .to_string(),
        ));
    }

    let definitions = translatable_definitions(schema);
    let definition_by_key = definitions
        .iter()
        .map(|definition| (definition.definition.field_key.as_str(), definition))
        .collect::<BTreeMap<_, _>>();

    let existing_target = load_exact_locale_values(
        db,
        entity.tenant_id,
        entity.entity_type,
        entity.entity_id,
        &request.target_locale,
    )
    .await
    .map_err(|error| FlexAttachedTranslationError::Database(error.to_string()))?
    .unwrap_or_else(empty_object);
    let mut full_target = existing_target
        .as_object()
        .cloned()
        .ok_or_else(|| {
            FlexAttachedTranslationError::OwnerInvariant(
                "Flex attached exact target storage must be a JSON object".to_string(),
            )
        })?;

    for target in &request.target_values {
        let definition = definition_by_key.get(target.field_key.as_str()).ok_or_else(|| {
            FlexAttachedTranslationError::Invalid(format!(
                "Flex attached translation field is not an active localized Text/Textarea leaf: {}",
                target.field_key
            ))
        })?;
        if definition.definition.is_required && target.value.trim().is_empty() {
            return Err(FlexAttachedTranslationError::Invalid(format!(
                "Flex attached required translation field must not be blank: {}",
                target.field_key
            )));
        }
        full_target.insert(target.field_key.clone(), Value::String(target.value.clone()));
    }

    let eligible_target = definitions
        .iter()
        .filter_map(|definition| {
            full_target
                .get(&definition.definition.field_key)
                .cloned()
                .map(|value| (definition.definition.field_key.clone(), value))
        })
        .collect::<Map<_, _>>();
    let eligible_schema = CustomFieldsSchema::new(
        definitions
            .iter()
            .map(|definition| definition.definition.clone())
            .collect(),
    );
    let validation_errors = eligible_schema.validate(&Value::Object(eligible_target));
    if !validation_errors.is_empty() {
        let message = validation_errors
            .into_iter()
            .map(|error| format!("{}: {}", error.field_key, error.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(FlexAttachedTranslationError::Invalid(format!(
            "Flex rejected translated attached values: {message}"
        )));
    }

    let desired = Value::Object(full_target);
    if desired != existing_target {
        persist_localized_values(
            db,
            entity.tenant_id,
            entity.entity_type,
            entity.entity_id,
            &request.target_locale,
            &desired,
        )
        .await
        .map_err(|error| FlexAttachedTranslationError::Database(error.to_string()))?;
    }

    let after = read_flex_attached_translation_exact(
        db,
        entity.clone(),
        schema,
        &request.source_locale,
        &request.target_locale,
    )
    .await?;
    let target_revision = after.target_revision.clone().ok_or_else(|| {
        FlexAttachedTranslationError::OwnerInvariant(
            "exact target locale is absent after Flex attached translation apply".to_string(),
        )
    })?;

    for target in &request.target_values {
        let actual = after
            .fields
            .iter()
            .find(|field| field.field_key == target.field_key)
            .and_then(|field| field.target_value.as_deref());
        if actual != Some(target.value.as_str()) {
            return Err(FlexAttachedTranslationError::OwnerInvariant(format!(
                "Flex attached translation apply did not persist exact target field {}",
                target.field_key
            )));
        }
    }

    Ok(FlexAttachedTranslationExactLocaleApplyReceipt {
        entity_type: entity.entity_type.to_string(),
        entity_id: entity.entity_id,
        resource_revision: after.resource_revision,
        target_revision,
        target_values: request.target_values.clone(),
    })
}

struct TranslatableDefinition<'a> {
    definition: &'a FieldDefinition,
    scalar_kind: FlexAttachedTranslationScalarKind,
}

fn translatable_definitions(schema: &CustomFieldsSchema) -> Vec<TranslatableDefinition<'_>> {
    let mut definitions = schema
        .active_definitions()
        .into_iter()
        .filter_map(|definition| {
            if !definition.is_localized {
                return None;
            }
            let scalar_kind = match definition.field_type {
                FieldType::Text => FlexAttachedTranslationScalarKind::Text,
                FieldType::Textarea => FlexAttachedTranslationScalarKind::Textarea,
                FieldType::Integer
                | FieldType::Decimal
                | FieldType::Boolean
                | FieldType::Date
                | FieldType::DateTime
                | FieldType::Url
                | FieldType::Email
                | FieldType::Phone
                | FieldType::Select
                | FieldType::MultiSelect
                | FieldType::Color
                | FieldType::Json => return None,
            };
            Some(TranslatableDefinition {
                definition,
                scalar_kind,
            })
        })
        .collect::<Vec<_>>();
    definitions.sort_by(|left, right| left.definition.field_key.cmp(&right.definition.field_key));
    definitions
}

fn build_snapshot(
    entity: AttachedEntityRef<'_>,
    definitions: &[TranslatableDefinition<'_>],
    rows: &[crate::attached::Model],
    source_locale: &str,
    target_locale: &str,
) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleSnapshot> {
    let eligible_keys = definitions
        .iter()
        .map(|definition| definition.definition.field_key.as_str())
        .collect::<BTreeSet<_>>();
    let mut values = BTreeMap::<String, BTreeMap<String, String>>::new();

    for row in rows {
        if !eligible_keys.contains(row.field_key.as_str()) {
            continue;
        }
        let Some(locale) = normalize_locale_tag(&row.locale) else {
            continue;
        };
        if locale == LEGACY_UNDETERMINED_LOCALE || locale != row.locale {
            continue;
        }
        let value = row.value.as_str().ok_or_else(|| {
            FlexAttachedTranslationError::OwnerInvariant(format!(
                "Flex attached localized Text/Textarea row must contain a string: {}/{}",
                row.locale, row.field_key
            ))
        })?;
        values
            .entry(locale)
            .or_default()
            .insert(row.field_key.clone(), value.to_string());
    }

    let source = values.get(source_locale);
    let target = values.get(target_locale);
    let mut fields = Vec::new();
    for definition in definitions {
        let source_value = source
            .and_then(|values| values.get(&definition.definition.field_key))
            .filter(|value| !value.trim().is_empty());
        if definition.definition.is_required && source_value.is_none() {
            return Err(FlexAttachedTranslationError::SourceLocaleNotFound {
                entity_type: entity.entity_type.to_string(),
                entity_id: entity.entity_id,
                locale: source_locale.to_string(),
            });
        }
        let Some(source_value) = source_value else {
            continue;
        };
        let target_value = target
            .and_then(|values| values.get(&definition.definition.field_key))
            .filter(|value| !value.trim().is_empty())
            .cloned();
        fields.push(FlexAttachedTranslationFieldSnapshot {
            field_key: definition.definition.field_key.clone(),
            scalar_kind: definition.scalar_kind,
            required: definition.definition.is_required,
            source_value: source_value.clone(),
            target_value,
        });
    }

    if fields.is_empty() {
        return Err(FlexAttachedTranslationError::SourceLocaleNotFound {
            entity_type: entity.entity_type.to_string(),
            entity_id: entity.entity_id,
            locale: source_locale.to_string(),
        });
    }

    let exact_locales = values.keys().cloned().collect::<Vec<_>>();
    let resource_revision = resource_revision(&entity, definitions, &values);
    let source_revision = locale_revision(
        &entity,
        definitions,
        source_locale,
        values.get(source_locale).expect("source-visible fields imply exact source rows"),
    );
    let target_revision = values.get(target_locale).map(|target| {
        locale_revision(&entity, definitions, target_locale, target)
    });

    Ok(FlexAttachedTranslationExactLocaleSnapshot {
        entity_type: entity.entity_type.to_string(),
        entity_id: entity.entity_id,
        source_locale: source_locale.to_string(),
        target_locale: target_locale.to_string(),
        resource_revision,
        source_revision,
        target_revision,
        exact_locales,
        fields,
    })
}

fn resource_revision(
    entity: &AttachedEntityRef<'_>,
    definitions: &[TranslatableDefinition<'_>],
    values: &BTreeMap<String, BTreeMap<String, String>>,
) -> String {
    let mut hasher = Sha256::new();
    digest(&mut hasher, RESOURCE_REVISION_NAMESPACE);
    digest(&mut hasher, &entity.tenant_id.to_string());
    digest(&mut hasher, entity.entity_type);
    digest(&mut hasher, &entity.entity_id.to_string());
    digest_schema(&mut hasher, definitions);
    for (locale, fields) in values {
        digest(&mut hasher, locale);
        for (field_key, value) in fields {
            digest(&mut hasher, field_key);
            digest(&mut hasher, value);
        }
    }
    format!("flex-attached-resource-v1:{}", hex::encode(hasher.finalize()))
}

fn locale_revision(
    entity: &AttachedEntityRef<'_>,
    definitions: &[TranslatableDefinition<'_>],
    locale: &str,
    values: &BTreeMap<String, String>,
) -> String {
    let mut hasher = Sha256::new();
    digest(&mut hasher, LOCALE_REVISION_NAMESPACE);
    digest(&mut hasher, &entity.tenant_id.to_string());
    digest(&mut hasher, entity.entity_type);
    digest(&mut hasher, &entity.entity_id.to_string());
    digest(&mut hasher, locale);
    digest_schema(&mut hasher, definitions);
    for (field_key, value) in values {
        digest(&mut hasher, field_key);
        digest(&mut hasher, value);
    }
    format!("flex-attached-locale-v1:{}", hex::encode(hasher.finalize()))
}

fn digest_schema(hasher: &mut Sha256, definitions: &[TranslatableDefinition<'_>]) {
    for definition in definitions {
        digest(hasher, &definition.definition.field_key);
        digest(
            hasher,
            match definition.scalar_kind {
                FlexAttachedTranslationScalarKind::Text => "text",
                FlexAttachedTranslationScalarKind::Textarea => "textarea",
            },
        );
        digest(
            hasher,
            if definition.definition.is_required {
                "required"
            } else {
                "optional"
            },
        );
        if let Some(validation) = &definition.definition.validation {
            digest_optional_f64(hasher, validation.min);
            digest_optional_f64(hasher, validation.max);
            digest(hasher, validation.pattern.as_deref().unwrap_or(""));
        } else {
            digest(hasher, "no-validation");
        }
    }
}

fn digest_optional_f64(hasher: &mut Sha256, value: Option<f64>) {
    match value {
        Some(value) => digest(hasher, &format!("{:016x}", value.to_bits())),
        None => digest(hasher, "none"),
    }
}

fn digest(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn ensure_revision(
    revision: &'static str,
    expected: &str,
    actual: &str,
) -> FlexAttachedTranslationResult<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(FlexAttachedTranslationError::RevisionConflict { revision })
    }
}

fn validate_entity(entity: &AttachedEntityRef<'_>) -> FlexAttachedTranslationResult<()> {
    if entity.tenant_id.is_nil() {
        return Err(FlexAttachedTranslationError::Invalid(
            "Flex attached translation tenant_id must not be nil".to_string(),
        ));
    }
    if entity.entity_id.is_nil() {
        return Err(FlexAttachedTranslationError::Invalid(
            "Flex attached translation entity_id must not be nil".to_string(),
        ));
    }
    if !is_valid_flex_entity_type(entity.entity_type) {
        return Err(FlexAttachedTranslationError::Invalid(format!(
            "Flex attached translation entity_type is invalid: {}",
            entity.entity_type
        )));
    }
    Ok(())
}

fn validate_authoring_locale(
    locale: &str,
    field: &str,
) -> FlexAttachedTranslationResult<()> {
    if locale == LEGACY_UNDETERMINED_LOCALE || normalize_locale_tag(locale).as_deref() != Some(locale)
    {
        return Err(FlexAttachedTranslationError::Invalid(format!(
            "Flex attached translation {field} must be a normalized authoring locale other than `und`"
        )));
    }
    Ok(())
}

fn validate_identity_component(
    value: &str,
    field: &str,
) -> FlexAttachedTranslationResult<()> {
    if value.trim().is_empty() || value.len() > 128 {
        return Err(FlexAttachedTranslationError::Invalid(format!(
            "Flex attached translation {field} must contain 1..=128 non-whitespace bytes"
        )));
    }
    Ok(())
}

fn validate_nonblank(value: &str, field: &str) -> FlexAttachedTranslationResult<()> {
    if value.trim().is_empty() {
        return Err(FlexAttachedTranslationError::Invalid(format!(
            "Flex attached translation {field} must not be blank"
        )));
    }
    Ok(())
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}

fn database_error(error: sea_orm::DbErr) -> FlexAttachedTranslationError {
    FlexAttachedTranslationError::Database(error.to_string())
}
