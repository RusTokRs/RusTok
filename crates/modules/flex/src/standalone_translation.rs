use async_trait::async_trait;
use rustok_api::{PortError, normalize_locale_tag};
use rustok_core::field_schema::{FieldDefinition, FieldType, is_valid_field_key};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};
use uuid::Uuid;

pub const MAX_FLEX_STANDALONE_TRANSLATION_RESOURCE_PAGE: u16 = 200;
const LEGACY_UNDETERMINED_LOCALE: &str = "und";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FlexStandaloneTranslationLeaf {
    pub field_key: String,
}

impl FlexStandaloneTranslationLeaf {
    pub fn validate(&self) -> FlexStandaloneTranslationResult<()> {
        if !is_valid_field_key(&self.field_key) {
            return Err(FlexStandaloneTranslationError::Invalid(format!(
                "Flex standalone translation field_key is invalid: {}",
                self.field_key
            )));
        }
        Ok(())
    }
}

pub fn flex_standalone_translation_field_eligible(definition: &FieldDefinition) -> bool {
    definition.is_active
        && definition.is_localized
        && matches!(definition.field_type, FieldType::Text | FieldType::Textarea)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexStandaloneTranslationLeafSnapshot {
    pub leaf: FlexStandaloneTranslationLeaf,
    pub required: bool,
    pub source_value: String,
    pub target_value: Option<String>,
}

impl FlexStandaloneTranslationLeafSnapshot {
    pub fn validate(&self) -> FlexStandaloneTranslationResult<()> {
        self.leaf.validate()?;
        validate_nonblank(&self.source_value, "source_value")?;
        if self
            .target_value
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(FlexStandaloneTranslationError::Invalid(
                "Flex standalone translation target values must be nonblank or null".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexStandaloneTranslationExactLocaleSnapshot {
    pub schema_id: Uuid,
    pub entry_id: Uuid,
    pub is_active: bool,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub leaves: Vec<FlexStandaloneTranslationLeafSnapshot>,
}

impl FlexStandaloneTranslationExactLocaleSnapshot {
    pub fn validate(&self) -> FlexStandaloneTranslationResult<()> {
        validate_uuid(self.schema_id, "schema_id")?;
        validate_uuid(self.entry_id, "entry_id")?;
        validate_flex_standalone_translation_locale_pair(&self.source_locale, &self.target_locale)?;
        validate_nonblank(&self.resource_revision, "resource_revision")?;
        validate_nonblank(&self.source_revision, "source_revision")?;
        if let Some(revision) = &self.target_revision {
            validate_nonblank(revision, "target_revision")?;
        }
        let mut locales = BTreeSet::new();
        for locale in &self.exact_locales {
            validate_authoring_locale(locale, "exact_locales")?;
            if !locales.insert(locale) {
                return Err(FlexStandaloneTranslationError::OwnerInvariant(
                    "Flex standalone translation exact locale inventory contains duplicates"
                        .to_string(),
                ));
            }
        }
        let mut leaves = BTreeSet::new();
        for leaf in &self.leaves {
            leaf.validate()?;
            if !leaves.insert(leaf.leaf.clone()) {
                return Err(FlexStandaloneTranslationError::OwnerInvariant(
                    "Flex standalone translation snapshot contains duplicate leaves".to_string(),
                ));
            }
        }
        if self.leaves.is_empty() {
            return Err(FlexStandaloneTranslationError::OwnerInvariant(
                "Flex standalone translation snapshot must expose at least one source leaf"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexStandaloneTranslationResourcePage {
    pub resources: Vec<FlexStandaloneTranslationExactLocaleSnapshot>,
    pub next_after: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexStandaloneTranslationExactProgress {
    pub required_units: u64,
    pub exact_required_units: u64,
    pub optional_units: u64,
    pub exact_optional_units: u64,
    pub resources: u64,
    pub complete_resources: u64,
}

impl FlexStandaloneTranslationExactProgress {
    pub fn validate(&self) -> FlexStandaloneTranslationResult<()> {
        if self.exact_required_units > self.required_units
            || self.exact_optional_units > self.optional_units
            || self.complete_resources > self.resources
        {
            return Err(FlexStandaloneTranslationError::OwnerInvariant(
                "Flex standalone translation aggregate progress is inconsistent".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexStandaloneTranslationTargetValue {
    pub leaf: FlexStandaloneTranslationLeaf,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexStandaloneTranslationOperationContext {
    pub idempotency_key: String,
    pub proposal_id: String,
    pub approval_receipt_id: String,
    pub request_fingerprint: String,
}

impl FlexStandaloneTranslationOperationContext {
    pub fn validate(&self) -> FlexStandaloneTranslationResult<()> {
        validate_nonblank(&self.idempotency_key, "idempotency_key")?;
        validate_nonblank(&self.proposal_id, "proposal_id")?;
        validate_nonblank(&self.approval_receipt_id, "approval_receipt_id")?;
        validate_nonblank(&self.request_fingerprint, "request_fingerprint")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlexStandaloneTranslationExactLocaleApply {
    pub operation: FlexStandaloneTranslationOperationContext,
    pub source_locale: String,
    pub target_locale: String,
    pub target_values: Vec<FlexStandaloneTranslationTargetValue>,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

impl FlexStandaloneTranslationExactLocaleApply {
    pub fn validate_admission(&self) -> FlexStandaloneTranslationResult<()> {
        self.operation.validate()?;
        validate_flex_standalone_translation_locale_pair(&self.source_locale, &self.target_locale)?;
        validate_nonblank(
            &self.expected_resource_revision,
            "expected_resource_revision",
        )?;
        validate_nonblank(&self.expected_source_revision, "expected_source_revision")?;
        if let Some(revision) = &self.expected_target_revision {
            validate_nonblank(revision, "expected_target_revision")?;
        }
        Ok(())
    }

    pub fn validate(&self) -> FlexStandaloneTranslationResult<()> {
        self.validate_admission()?;
        if self.target_values.is_empty() {
            return Err(FlexStandaloneTranslationError::Invalid(
                "Flex standalone translation apply must contain at least one target value"
                    .to_string(),
            ));
        }
        let mut leaves = BTreeSet::new();
        for target in &self.target_values {
            target.leaf.validate()?;
            if !leaves.insert(target.leaf.clone()) {
                return Err(FlexStandaloneTranslationError::Invalid(
                    "Flex standalone translation apply contains a duplicate leaf".to_string(),
                ));
            }
            if target
                .value
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
            {
                return Err(FlexStandaloneTranslationError::Invalid(
                    "Flex standalone translation target values must be nonblank or null"
                        .to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexStandaloneTranslationExactLocaleApplyReceipt {
    pub operation_id: Uuid,
    pub schema_id: Uuid,
    pub entry_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target_values: Vec<FlexStandaloneTranslationTargetValue>,
}

#[derive(Debug)]
pub enum FlexStandaloneTranslationError {
    Invalid(String),
    EntryNotFound {
        schema_id: Uuid,
        entry_id: Uuid,
    },
    SourceLocaleNotFound {
        schema_id: Uuid,
        entry_id: Uuid,
        locale: String,
    },
    RevisionConflict {
        revision: &'static str,
    },
    Operation(PortError),
    Storage(String),
    OwnerInvariant(String),
}

impl fmt::Display for FlexStandaloneTranslationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::EntryNotFound {
                schema_id,
                entry_id,
            } => {
                write!(
                    formatter,
                    "Flex standalone entry not found: {schema_id}/{entry_id}"
                )
            }
            Self::SourceLocaleNotFound {
                schema_id,
                entry_id,
                locale,
            } => write!(
                formatter,
                "Flex standalone source locale not found: {locale} for {schema_id}/{entry_id}"
            ),
            Self::RevisionConflict { revision } => write!(
                formatter,
                "Flex standalone translation {revision} revision conflict"
            ),
            Self::Operation(error) => write!(
                formatter,
                "Flex standalone translation operation failed: {error}"
            ),
            Self::Storage(message) => {
                write!(
                    formatter,
                    "Flex standalone translation storage error: {message}"
                )
            }
            Self::OwnerInvariant(message) => write!(
                formatter,
                "Flex standalone translation owner invariant failed: {message}"
            ),
        }
    }
}

impl std::error::Error for FlexStandaloneTranslationError {}

pub type FlexStandaloneTranslationResult<T> = Result<T, FlexStandaloneTranslationError>;

pub fn validate_flex_standalone_translation_resource_page(
    limit: u16,
) -> FlexStandaloneTranslationResult<()> {
    if limit == 0 || limit > MAX_FLEX_STANDALONE_TRANSLATION_RESOURCE_PAGE {
        return Err(FlexStandaloneTranslationError::Invalid(format!(
            "Flex standalone translation resource page size must be between 1 and {MAX_FLEX_STANDALONE_TRANSLATION_RESOURCE_PAGE}"
        )));
    }
    Ok(())
}

pub fn validate_flex_standalone_translation_locale_pair(
    source_locale: &str,
    target_locale: &str,
) -> FlexStandaloneTranslationResult<()> {
    validate_authoring_locale(source_locale, "source_locale")?;
    validate_authoring_locale(target_locale, "target_locale")?;
    if source_locale == target_locale {
        return Err(FlexStandaloneTranslationError::Invalid(
            "Flex standalone translation source and target locale must differ".to_string(),
        ));
    }
    Ok(())
}

fn validate_authoring_locale(locale: &str, field: &str) -> FlexStandaloneTranslationResult<()> {
    if locale == LEGACY_UNDETERMINED_LOCALE
        || normalize_locale_tag(locale).as_deref() != Some(locale)
    {
        return Err(FlexStandaloneTranslationError::Invalid(format!(
            "Flex standalone translation {field} must be a normalized authoring locale other than `und`"
        )));
    }
    Ok(())
}

fn validate_nonblank(value: &str, field: &str) -> FlexStandaloneTranslationResult<()> {
    if value.trim().is_empty() {
        return Err(FlexStandaloneTranslationError::Invalid(format!(
            "Flex standalone translation {field} must not be blank"
        )));
    }
    Ok(())
}

fn validate_uuid(value: Uuid, field: &str) -> FlexStandaloneTranslationResult<()> {
    if value.is_nil() {
        return Err(FlexStandaloneTranslationError::Invalid(format!(
            "Flex standalone translation {field} must not be the nil UUID"
        )));
    }
    Ok(())
}

#[async_trait]
pub trait FlexStandaloneTranslationOwnerPort: Send + Sync {
    async fn list_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<Uuid>,
        limit: u16,
    ) -> FlexStandaloneTranslationResult<FlexStandaloneTranslationResourcePage>;

    async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
        entry_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexStandaloneTranslationResult<FlexStandaloneTranslationExactLocaleSnapshot>;

    async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        schema_id: Uuid,
        entry_id: Uuid,
        request: FlexStandaloneTranslationExactLocaleApply,
    ) -> FlexStandaloneTranslationResult<FlexStandaloneTranslationExactLocaleApplyReceipt>;
}

#[async_trait]
pub trait FlexStandaloneTranslationProgressOwnerPort: Send + Sync {
    async fn read_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexStandaloneTranslationResult<FlexStandaloneTranslationExactProgress>;
}
