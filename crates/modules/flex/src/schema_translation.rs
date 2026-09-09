use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

pub const MAX_FLEX_SCHEMA_TRANSLATION_RESOURCE_PAGE: u16 = 200;

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
    FieldLabel {
        field_key: String,
    },
    FieldDescription {
        field_key: String,
    },
    FieldValidationErrorMessage {
        field_key: String,
    },
    FieldOptionLabel {
        field_key: String,
        option_index: u16,
        option_value: String,
    },
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
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlexSchemaTranslationExactLocaleApply {
    pub source_locale: String,
    pub target_locale: String,
    /// Complete target values for the exact source-visible leaf set returned by the
    /// owner snapshot. The owner rejects unknown, duplicate, or omitted leaves.
    pub target_values: Vec<FlexSchemaTranslationTargetValue>,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexSchemaTranslationExactLocaleApplyReceipt {
    pub operation_id: Option<Uuid>,
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
            Self::Database(message) => write!(formatter, "Flex schema translation database error: {message}"),
            Self::OwnerInvariant(message) => {
                write!(formatter, "Flex schema translation owner invariant failed: {message}")
            }
        }
    }
}

impl std::error::Error for FlexSchemaTranslationError {}

pub type FlexSchemaTranslationResult<T> = Result<T, FlexSchemaTranslationError>;

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
