use async_trait::async_trait;
use rustok_api::{PortError, normalize_locale_tag};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};
use uuid::Uuid;

pub const MAX_FLEX_SCHEMA_TRANSLATION_RESOURCE_PAGE: u16 = 200;
const LEGACY_UNDETERMINED_LOCALE: &str = "und";

/// A structurally declared localized presentation leaf inside one standalone Flex schema.
///
/// The contract deliberately enumerates only schema copy and the localized maps that
/// `FieldDefinition` explicitly owns. Arbitrary strings from `settings`, defaults,
/// patterns, option values, or entry payloads can never become translation fields by
/// inference.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FlexSchemaTranslationLeaf {
    SchemaName,
    SchemaDescription,
    FieldLabel { field_key: String },
    FieldDescription { field_key: String },
    FieldValidationErrorMessage { field_key: String },
    FieldOptionLabel {
        field_key: String,
        /// Select option values are validated as unique within one field and therefore
        /// form the stable leaf identity. Array position is intentionally excluded so
        /// option reordering cannot rename Translation work.
        option_value: String,
    },
}

impl FlexSchemaTranslationLeaf {
    fn validate(&self) -> FlexSchemaTranslationResult<()> {
        match self {
            Self::SchemaName | Self::SchemaDescription => Ok(()),
            Self::FieldLabel { field_key }
            | Self::FieldDescription { field_key }
            | Self::FieldValidationErrorMessage { field_key } => {
                validate_identity_component(field_key, "field_key")
            }
            Self::FieldOptionLabel {
                field_key,
                option_value,
            } => {
                validate_identity_component(field_key, "field_key")?;
                validate_identity_component(option_value, "option_value")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexSchemaTranslationLeafSnapshot {
    pub leaf: FlexSchemaTranslationLeaf,
    pub source_value: String,
    pub target_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexSchemaTranslationExactLocaleSnapshot {
    pub schema_id: Uuid,
    pub slug: String,
    pub is_active: bool,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    /// Only leaves with exact source-locale content are exposed. This prevents
    /// Translation from manufacturing copy for undeclared or source-absent JSON paths.
    pub leaves: Vec<FlexSchemaTranslationLeafSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexSchemaTranslationResourcePage {
    pub resources: Vec<FlexSchemaTranslationExactLocaleSnapshot>,
    pub next_after: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexSchemaTranslationTargetValue {
    pub leaf: FlexSchemaTranslationLeaf,
    /// `None` means remove the exact target-locale value for an optional leaf. Empty
    /// strings are deliberately not a second spelling of absence.
    pub value: Option<String>,
}

/// Translation workflow evidence that participates in durable owner admission.
///
/// `request_fingerprint` is the canonical fingerprint of the original neutral patch,
/// not a hash of the live/derived merged target state. That distinction keeps retries
/// bound to the user's original proposal even if unrelated target copy changes later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexSchemaTranslationOperationContext {
    pub idempotency_key: String,
    pub proposal_id: String,
    pub approval_receipt_id: String,
    pub request_fingerprint: String,
}

impl FlexSchemaTranslationOperationContext {
    pub fn validate(&self) -> FlexSchemaTranslationResult<()> {
        validate_nonblank(&self.idempotency_key, "idempotency_key")?;
        validate_nonblank(&self.proposal_id, "proposal_id")?;
        validate_nonblank(&self.approval_receipt_id, "approval_receipt_id")?;
        validate_nonblank(&self.request_fingerprint, "request_fingerprint")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlexSchemaTranslationExactLocaleApply {
    pub operation: FlexSchemaTranslationOperationContext,
    pub source_locale: String,
    pub target_locale: String,
    /// Complete target values for the exact source-visible leaf set returned by the
    /// owner snapshot. The owner rejects unknown, duplicate, or omitted leaves.
    pub target_values: Vec<FlexSchemaTranslationTargetValue>,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

impl FlexSchemaTranslationExactLocaleApply {
    /// Validate the immutable request identity used before durable owner admission.
    /// Derived target values are deliberately excluded from this phase.
    pub fn validate_admission(&self) -> FlexSchemaTranslationResult<()> {
        self.operation.validate()?;
        validate_flex_schema_translation_locale_pair(&self.source_locale, &self.target_locale)?;
        validate_nonblank(&self.expected_resource_revision, "expected_resource_revision")?;
        validate_nonblank(&self.expected_source_revision, "expected_source_revision")?;
        if let Some(revision) = &self.expected_target_revision {
            validate_nonblank(revision, "expected_target_revision")?;
        }
        Ok(())
    }

    pub fn validate(&self) -> FlexSchemaTranslationResult<()> {
        self.validate_admission()?;
        if self.target_values.is_empty() {
            return Err(FlexSchemaTranslationError::Invalid(
                "Flex schema translation apply must contain at least one target value".to_string(),
            ));
        }

        let mut leaves = BTreeSet::new();
        for target in &self.target_values {
            target.leaf.validate()?;
            if !leaves.insert(target.leaf.clone()) {
                return Err(FlexSchemaTranslationError::Invalid(
                    "Flex schema translation apply contains a duplicate leaf".to_string(),
                ));
            }
            if target.value.as_ref().is_some_and(|value| value.trim().is_empty()) {
                return Err(FlexSchemaTranslationError::Invalid(
                    "Flex schema translation target values must be nonblank or null".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexSchemaTranslationExactLocaleApplyReceipt {
    /// Provider-facing applies always run under a durable owner operation lease, so
    /// receipt identity is mandatory rather than best-effort.
    pub operation_id: Uuid,
    pub schema_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target_values: Vec<FlexSchemaTranslationTargetValue>,
}

#[derive(Debug)]
pub enum FlexSchemaTranslationError {
    Invalid(String),
    SchemaNotFound(Uuid),
    SourceLocaleNotFound { schema_id: Uuid, locale: String },
    RevisionConflict { revision: &'static str },
    /// Preserve the platform port error shape so the neutral provider can retain
    /// conflict/retryability semantics from durable owner idempotency admission.
    Operation(PortError),
    Database(String),
    OwnerInvariant(String),
}

impl fmt::Display for FlexSchemaTranslationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::SchemaNotFound(schema_id) => {
                write!(formatter, "Flex schema not found: {schema_id}")
            }
            Self::SourceLocaleNotFound { schema_id, locale } => write!(
                formatter,
                "Flex schema source locale not found: {locale} for schema {schema_id}"
            ),
            Self::RevisionConflict { revision } => {
                write!(formatter, "Flex schema translation {revision} revision conflict")
            }
            Self::Operation(error) => {
                write!(formatter, "Flex schema translation operation failed: {error}")
            }
            Self::Database(message) => {
                write!(formatter, "Flex schema translation database error: {message}")
            }
            Self::OwnerInvariant(message) => {
                write!(formatter, "Flex schema translation owner invariant failed: {message}")
            }
        }
    }
}

impl std::error::Error for FlexSchemaTranslationError {}

pub type FlexSchemaTranslationResult<T> = Result<T, FlexSchemaTranslationError>;

pub fn validate_flex_schema_translation_resource_page(
    limit: u16,
) -> FlexSchemaTranslationResult<()> {
    if limit == 0 || limit > MAX_FLEX_SCHEMA_TRANSLATION_RESOURCE_PAGE {
        return Err(FlexSchemaTranslationError::Invalid(format!(
            "Flex schema translation resource page size must be between 1 and {MAX_FLEX_SCHEMA_TRANSLATION_RESOURCE_PAGE}"
        )));
    }
    Ok(())
}

pub fn validate_flex_schema_translation_locale_pair(
    source_locale: &str,
    target_locale: &str,
) -> FlexSchemaTranslationResult<()> {
    validate_authoring_locale(source_locale, "source_locale")?;
    validate_authoring_locale(target_locale, "target_locale")?;
    if source_locale == target_locale {
        return Err(FlexSchemaTranslationError::Invalid(
            "Flex schema translation source and target locale must differ".to_string(),
        ));
    }
    Ok(())
}

fn validate_authoring_locale(locale: &str, field: &str) -> FlexSchemaTranslationResult<()> {
    if locale == LEGACY_UNDETERMINED_LOCALE || normalize_locale_tag(locale).as_deref() != Some(locale)
    {
        return Err(FlexSchemaTranslationError::Invalid(format!(
            "Flex schema translation {field} must be a normalized authoring locale other than `und`"
        )));
    }
    Ok(())
}

fn validate_identity_component(value: &str, field: &str) -> FlexSchemaTranslationResult<()> {
    if value.trim().is_empty() || value.len() > 128 {
        return Err(FlexSchemaTranslationError::Invalid(format!(
            "Flex schema translation {field} must contain 1..=128 non-whitespace bytes"
        )));
    }
    Ok(())
}

fn validate_nonblank(value: &str, field: &str) -> FlexSchemaTranslationResult<()> {
    if value.trim().is_empty() {
        return Err(FlexSchemaTranslationError::Invalid(format!(
            "Flex schema translation {field} must not be blank"
        )));
    }
    Ok(())
}

/// Provider-facing exact-locale owner port for standalone Flex schema copy.
///
/// Persistence remains a host adapter concern. Translation and the neutral target
/// provider can only see this exact-locale/revision-safe owner contract and therefore
/// cannot read or mutate `flex_schemas`, `flex_schema_translations`, or `fields_config`
/// directly.
#[async_trait]
pub trait FlexSchemaTranslationOwnerPort: Send + Sync {
    async fn list_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<Uuid>,
        limit: u16,
    ) -> FlexSchemaTranslationResult<FlexSchemaTranslationResourcePage>;

    async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexSchemaTranslationResult<FlexSchemaTranslationExactLocaleSnapshot>;

    async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        schema_id: Uuid,
        request: FlexSchemaTranslationExactLocaleApply,
    ) -> FlexSchemaTranslationResult<FlexSchemaTranslationExactLocaleApplyReceipt>;
}
