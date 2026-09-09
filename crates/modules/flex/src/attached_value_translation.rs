use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use async_trait::async_trait;
use rustok_api::{PortError, normalize_locale_tag};
use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FieldType};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::is_valid_flex_entity_type;

pub const MAX_FLEX_ATTACHED_VALUE_TRANSLATION_RESOURCE_PAGE: u16 = 200;
const RESOURCE_REVISION_NAMESPACE: &str = "rustok-flex/attached-localized-value-resource/v1";
const LOCALE_REVISION_NAMESPACE: &str = "rustok-flex/attached-localized-value-locale/v1";
const LEGACY_UNDETERMINED_LOCALE: &str = "und";

/// Stable identity for one Flex-owned attached-value donor resource.
///
/// `entity_type` is part of the identity because attached storage is intentionally shared
/// by registered donors. Translation must never infer or scan foreign owner tables.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FlexAttachedValueTranslationResourceId {
    pub entity_type: String,
    pub entity_id: Uuid,
}

impl FlexAttachedValueTranslationResourceId {
    pub fn validate(&self) -> FlexAttachedValueTranslationResult<()> {
        if !is_valid_flex_entity_type(&self.entity_type) {
            return Err(FlexAttachedValueTranslationError::Invalid(
                "Flex attached-value translation entity_type must satisfy the registered namespaced donor contract"
                    .to_string(),
            ));
        }
        validate_uuid(self.entity_id, "entity_id")
    }
}

/// One schema-declared localized scalar value that Translation may own.
///
/// Only the field key is exposed. Arbitrary JSON paths, select option values, and inferred
/// strings are deliberately impossible to represent in this contract.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FlexAttachedValueTranslationLeaf {
    pub field_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedValueTranslationLeafSnapshot {
    pub leaf: FlexAttachedValueTranslationLeaf,
    pub required: bool,
    pub source_value: String,
    pub target_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedValueTranslationExactLocaleSnapshot {
    pub resource: FlexAttachedValueTranslationResourceId,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    /// Only exact source-locale values for explicitly localized Text/Textarea fields are
    /// exposed. Missing/blank source values do not create Translation work.
    pub leaves: Vec<FlexAttachedValueTranslationLeafSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedValueTranslationResourcePage {
    pub resources: Vec<FlexAttachedValueTranslationExactLocaleSnapshot>,
    pub next_after: Option<FlexAttachedValueTranslationResourceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedValueTranslationTargetValue {
    pub leaf: FlexAttachedValueTranslationLeaf,
    /// `None` removes an optional exact target-locale value. Required source-visible leaves
    /// must always receive a nonblank exact target value.
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedValueTranslationOperationContext {
    pub idempotency_key: String,
    pub proposal_id: String,
    pub approval_receipt_id: String,
    pub request_fingerprint: String,
}

impl FlexAttachedValueTranslationOperationContext {
    pub fn validate(&self) -> FlexAttachedValueTranslationResult<()> {
        validate_nonblank(&self.idempotency_key, "idempotency_key")?;
        validate_nonblank(&self.proposal_id, "proposal_id")?;
        validate_nonblank(&self.approval_receipt_id, "approval_receipt_id")?;
        validate_nonblank(&self.request_fingerprint, "request_fingerprint")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlexAttachedValueTranslationExactLocaleApply {
    pub operation: FlexAttachedValueTranslationOperationContext,
    pub source_locale: String,
    pub target_locale: String,
    pub target_values: Vec<FlexAttachedValueTranslationTargetValue>,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

impl FlexAttachedValueTranslationExactLocaleApply {
    pub fn validate_admission(&self) -> FlexAttachedValueTranslationResult<()> {
        self.operation.validate()?;
        validate_locale_pair(&self.source_locale, &self.target_locale)?;
        validate_nonblank(&self.expected_resource_revision, "expected_resource_revision")?;
        validate_nonblank(&self.expected_source_revision, "expected_source_revision")?;
        if let Some(revision) = &self.expected_target_revision {
            validate_nonblank(revision, "expected_target_revision")?;
        }
        Ok(())
    }

    pub fn validate(&self) -> FlexAttachedValueTranslationResult<()> {
        self.validate_admission()?;
        if self.target_values.is_empty() {
            return Err(FlexAttachedValueTranslationError::Invalid(
                "Flex attached-value translation apply must contain at least one target value"
                    .to_string(),
            ));
        }
        let mut leaves = BTreeSet::new();
        for target in &self.target_values {
            validate_field_key(&target.leaf.field_key)?;
            if !leaves.insert(target.leaf.clone()) {
                return Err(FlexAttachedValueTranslationError::Invalid(
                    "Flex attached-value translation apply contains a duplicate leaf".to_string(),
                ));
            }
            if target.value.as_ref().is_some_and(|value| value.trim().is_empty()) {
                return Err(FlexAttachedValueTranslationError::Invalid(
                    "Flex attached-value translation target values must be nonblank or null"
                        .to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedValueTranslationExactLocaleApplyReceipt {
    pub operation_id: Uuid,
    pub resource: FlexAttachedValueTranslationResourceId,
    pub resource_revision: String,
    pub target_revision: String,
    pub target_values: Vec<FlexAttachedValueTranslationTargetValue>,
}

#[derive(Debug)]
pub enum FlexAttachedValueTranslationError {
    Invalid(String),
    ResourceNotFound(FlexAttachedValueTranslationResourceId),
    SourceLocaleNotFound {
        resource: FlexAttachedValueTranslationResourceId,
        locale: String,
    },
    RevisionConflict { revision: &'static str },
    Operation(PortError),
    Database(String),
    OwnerInvariant(String),
}

impl fmt::Display for FlexAttachedValueTranslationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::ResourceNotFound(resource) => write!(
                formatter,
                "Flex attached-value resource not found: {}:{}",
                resource.entity_type, resource.entity_id
            ),
            Self::SourceLocaleNotFound { resource, locale } => write!(
                formatter,
                "Flex attached-value source locale not found: {locale} for {}:{}",
                resource.entity_type, resource.entity_id
            ),
            Self::RevisionConflict { revision } => write!(
                formatter,
                "Flex attached-value translation {revision} revision conflict"
            ),
            Self::Operation(error) => {
                write!(formatter, "Flex attached-value translation operation failed: {error}")
            }
            Self::Database(message) => write!(
                formatter,
                "Flex attached-value translation database error: {message}"
            ),
            Self::OwnerInvariant(message) => write!(
                formatter,
                "Flex attached-value translation owner invariant failed: {message}"
            ),
        }
    }
}

impl std::error::Error for FlexAttachedValueTranslationError {}

pub type FlexAttachedValueTranslationResult<T> =
    Result<T, FlexAttachedValueTranslationError>;

/// Value-Translation eligibility is intentionally narrower than generic Flex localization.
///
/// Select values are stable machine identities whose labels belong to schema copy; URLs,
/// emails, phone numbers, colors, numerics, booleans, dates, and JSON are not linguistic
/// copy. This leaves only explicitly localized active Text/Textarea fields.
pub fn flex_attached_value_translation_definition_eligible(definition: &FieldDefinition) -> bool {
    definition.is_active
        && definition.is_localized
        && matches!(definition.field_type, FieldType::Text | FieldType::Textarea)
}

pub fn flex_attached_value_translation_definitions(
    schema: &CustomFieldsSchema,
) -> Vec<FieldDefinition> {
    let mut definitions = schema
        .active_definitions()
        .into_iter()
        .filter(|definition| flex_attached_value_translation_definition_eligible(definition))
        .cloned()
        .collect::<Vec<_>>();
    definitions.sort_by(|left, right| left.field_key.cmp(&right.field_key));
    definitions
}

/// Build the deterministic exact-locale owner snapshot from authoritative schema and exact
/// Flex-owned localized rows. Callers must validate the donor identity before invoking it.
pub fn build_flex_attached_value_translation_snapshot(
    resource: FlexAttachedValueTranslationResourceId,
    schema: &CustomFieldsSchema,
    localized_by_locale: &BTreeMap<String, Map<String, Value>>,
    source_locale: &str,
    target_locale: &str,
) -> FlexAttachedValueTranslationResult<FlexAttachedValueTranslationExactLocaleSnapshot> {
    resource.validate()?;
    validate_locale_pair(source_locale, target_locale)?;

    let definitions = flex_attached_value_translation_definitions(schema);
    if definitions.is_empty() {
        return Err(FlexAttachedValueTranslationError::SourceLocaleNotFound {
            resource,
            locale: source_locale.to_string(),
        });
    }

    let normalized = normalize_locales(localized_by_locale)?;
    let source_values = normalized.get(source_locale).cloned().unwrap_or_default();
    let target_values = normalized.get(target_locale).cloned().unwrap_or_default();

    let mut leaves = Vec::new();
    for definition in &definitions {
        let Some(source_value) = exact_translation_string(&source_values, &definition.field_key)?
        else {
            continue;
        };
        leaves.push(FlexAttachedValueTranslationLeafSnapshot {
            leaf: FlexAttachedValueTranslationLeaf {
                field_key: definition.field_key.clone(),
            },
            required: definition.is_required,
            source_value,
            target_value: exact_translation_string(&target_values, &definition.field_key)?,
        });
    }
    if leaves.is_empty() {
        return Err(FlexAttachedValueTranslationError::SourceLocaleNotFound {
            resource,
            locale: source_locale.to_string(),
        });
    }

    let resource_revision = attached_resource_revision(&resource, &definitions, &normalized)?;
    let source_revision = attached_locale_revision(
        &resource,
        &definitions,
        source_locale,
        normalized.get(source_locale),
    )?;
    let target_revision = normalized
        .get(target_locale)
        .filter(|values| locale_has_translation_values(values, &definitions))
        .map(|values| {
            attached_locale_revision(
                &resource,
                &definitions,
                target_locale,
                Some(values),
            )
        })
        .transpose()?;

    let exact_locales = normalized
        .iter()
        .filter(|(_, values)| locale_has_translation_values(values, &definitions))
        .map(|(locale, _)| locale.clone())
        .collect();

    Ok(FlexAttachedValueTranslationExactLocaleSnapshot {
        resource,
        source_locale: source_locale.to_string(),
        target_locale: target_locale.to_string(),
        resource_revision,
        source_revision,
        target_revision,
        exact_locales,
        leaves,
    })
}

#[async_trait]
pub trait FlexAttachedValueTranslationOwnerPort: Send + Sync {
    async fn list_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<FlexAttachedValueTranslationResourceId>,
        limit: u16,
    ) -> FlexAttachedValueTranslationResult<FlexAttachedValueTranslationResourcePage>;

    async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        resource: FlexAttachedValueTranslationResourceId,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexAttachedValueTranslationResult<FlexAttachedValueTranslationExactLocaleSnapshot>;

    async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        resource: FlexAttachedValueTranslationResourceId,
        request: FlexAttachedValueTranslationExactLocaleApply,
    ) -> FlexAttachedValueTranslationResult<FlexAttachedValueTranslationExactLocaleApplyReceipt>;
}

pub fn validate_flex_attached_value_translation_resource_page(
    limit: u16,
) -> FlexAttachedValueTranslationResult<()> {
    if limit == 0 || limit > MAX_FLEX_ATTACHED_VALUE_TRANSLATION_RESOURCE_PAGE {
        return Err(FlexAttachedValueTranslationError::Invalid(format!(
            "Flex attached-value translation resource page size must be between 1 and {MAX_FLEX_ATTACHED_VALUE_TRANSLATION_RESOURCE_PAGE}"
        )));
    }
    Ok(())
}

fn normalize_locales(
    localized_by_locale: &BTreeMap<String, Map<String, Value>>,
) -> FlexAttachedValueTranslationResult<BTreeMap<String, Map<String, Value>>> {
    let mut normalized = BTreeMap::new();
    for (locale, values) in localized_by_locale {
        let canonical = normalize_locale_tag(locale).ok_or_else(|| {
            FlexAttachedValueTranslationError::OwnerInvariant(format!(
                "persisted Flex attached locale is invalid: {locale}"
            ))
        })?;
        if canonical == LEGACY_UNDETERMINED_LOCALE {
            continue;
        }
        if normalized.insert(canonical.clone(), values.clone()).is_some() {
            return Err(FlexAttachedValueTranslationError::OwnerInvariant(format!(
                "persisted Flex attached locales normalize to duplicate locale: {canonical}"
            )));
        }
    }
    Ok(normalized)
}

fn exact_translation_string(
    values: &Map<String, Value>,
    field_key: &str,
) -> FlexAttachedValueTranslationResult<Option<String>> {
    match values.get(field_key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.trim().is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(FlexAttachedValueTranslationError::OwnerInvariant(format!(
            "persisted localized Text/Textarea field `{field_key}` is not a string"
        ))),
    }
}

fn locale_has_translation_values(
    values: &Map<String, Value>,
    definitions: &[FieldDefinition],
) -> bool {
    definitions.iter().any(|definition| {
        values
            .get(&definition.field_key)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    })
}

fn attached_resource_revision(
    resource: &FlexAttachedValueTranslationResourceId,
    definitions: &[FieldDefinition],
    localized_by_locale: &BTreeMap<String, Map<String, Value>>,
) -> FlexAttachedValueTranslationResult<String> {
    let mut hasher = Sha256::new();
    digest(&mut hasher, RESOURCE_REVISION_NAMESPACE);
    digest(&mut hasher, &resource.entity_type);
    digest(&mut hasher, &resource.entity_id.to_string());
    digest_definitions(&mut hasher, definitions);
    for (locale, values) in localized_by_locale {
        digest(&mut hasher, locale);
        digest_locale_values(&mut hasher, values, definitions)?;
    }
    Ok(format!(
        "flex-attached-value-resource-v1:{}",
        hex::encode(hasher.finalize())
    ))
}

fn attached_locale_revision(
    resource: &FlexAttachedValueTranslationResourceId,
    definitions: &[FieldDefinition],
    locale: &str,
    values: Option<&Map<String, Value>>,
) -> FlexAttachedValueTranslationResult<String> {
    let mut hasher = Sha256::new();
    digest(&mut hasher, LOCALE_REVISION_NAMESPACE);
    digest(&mut hasher, &resource.entity_type);
    digest(&mut hasher, &resource.entity_id.to_string());
    digest(&mut hasher, locale);
    digest_definitions(&mut hasher, definitions);
    if let Some(values) = values {
        digest_locale_values(&mut hasher, values, definitions)?;
    }
    Ok(format!(
        "flex-attached-value-locale-v1:{}",
        hex::encode(hasher.finalize())
    ))
}

fn digest_definitions(hasher: &mut Sha256, definitions: &[FieldDefinition]) {
    for definition in definitions {
        digest(hasher, &definition.field_key);
        digest(
            hasher,
            match definition.field_type {
                FieldType::Text => "text",
                FieldType::Textarea => "textarea",
                _ => "ineligible",
            },
        );
        digest(hasher, if definition.is_required { "required" } else { "optional" });
        if let Some(validation) = &definition.validation {
            digest(
                hasher,
                &validation.min.map(|value| value.to_bits()).unwrap_or_default().to_string(),
            );
            digest(
                hasher,
                &validation.max.map(|value| value.to_bits()).unwrap_or_default().to_string(),
            );
            digest(hasher, validation.pattern.as_deref().unwrap_or(""));
        } else {
            digest(hasher, "");
            digest(hasher, "");
            digest(hasher, "");
        }
    }
}

fn digest_locale_values(
    hasher: &mut Sha256,
    values: &Map<String, Value>,
    definitions: &[FieldDefinition],
) -> FlexAttachedValueTranslationResult<()> {
    for definition in definitions {
        digest(hasher, &definition.field_key);
        match exact_translation_string(values, &definition.field_key)? {
            Some(value) => {
                digest(hasher, "present");
                digest(hasher, &value);
            }
            None => digest(hasher, "absent"),
        }
    }
    Ok(())
}

fn validate_locale_pair(
    source_locale: &str,
    target_locale: &str,
) -> FlexAttachedValueTranslationResult<()> {
    validate_authoring_locale(source_locale, "source_locale")?;
    validate_authoring_locale(target_locale, "target_locale")?;
    if source_locale == target_locale {
        return Err(FlexAttachedValueTranslationError::Invalid(
            "Flex attached-value translation source and target locale must differ".to_string(),
        ));
    }
    Ok(())
}

fn validate_authoring_locale(
    locale: &str,
    field: &str,
) -> FlexAttachedValueTranslationResult<()> {
    if locale == LEGACY_UNDETERMINED_LOCALE || normalize_locale_tag(locale).as_deref() != Some(locale)
    {
        return Err(FlexAttachedValueTranslationError::Invalid(format!(
            "Flex attached-value translation {field} must be a normalized authoring locale other than `und`"
        )));
    }
    Ok(())
}

fn validate_uuid(value: Uuid, field: &str) -> FlexAttachedValueTranslationResult<()> {
    if value.is_nil() {
        return Err(FlexAttachedValueTranslationError::Invalid(format!(
            "Flex attached-value translation {field} must not be nil"
        )));
    }
    Ok(())
}

fn validate_field_key(value: &str) -> FlexAttachedValueTranslationResult<()> {
    if value.trim().is_empty() || value.len() > 128 {
        return Err(FlexAttachedValueTranslationError::Invalid(
            "Flex attached-value translation field_key must contain 1..=128 non-whitespace bytes"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_nonblank(value: &str, field: &str) -> FlexAttachedValueTranslationResult<()> {
    if value.trim().is_empty() {
        return Err(FlexAttachedValueTranslationError::Invalid(format!(
            "Flex attached-value translation {field} must not be blank"
        )));
    }
    Ok(())
}

fn digest(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}
