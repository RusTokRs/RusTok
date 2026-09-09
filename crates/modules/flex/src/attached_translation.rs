use async_trait::async_trait;
use rustok_api::{PortError, normalize_locale_tag};
use rustok_core::field_schema::{FieldDefinition, FieldType, is_valid_field_key};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};
use uuid::Uuid;

use crate::entity_type::normalize_flex_entity_type;

pub const MAX_FLEX_ATTACHED_TRANSLATION_RESOURCE_PAGE: u16 = 200;
const LEGACY_UNDETERMINED_LOCALE: &str = "und";

/// Stable translatable leaf owned by one attached Flex donor.
///
/// Attached storage can persist arbitrary JSON values, but Translation must never infer
/// translatable copy from arbitrary JSON. Only explicitly localized text/textarea field
/// definitions are eligible and the stable `field_key` is the leaf identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FlexAttachedTranslationLeaf {
    pub field_key: String,
}

impl FlexAttachedTranslationLeaf {
    pub fn validate(&self) -> FlexAttachedTranslationResult<()> {
        if !is_valid_field_key(&self.field_key) {
            return Err(FlexAttachedTranslationError::Invalid(format!(
                "Flex attached translation field_key is invalid: {}",
                self.field_key
            )));
        }
        Ok(())
    }
}

/// Whether a Flex definition may be exposed as attached-value translation copy.
///
/// `is_localized` controls locale ownership, while the field type controls whether the
/// stored scalar is linguistic copy. Select values, URLs, email addresses, phone numbers,
/// numbers, booleans, dates, colors and JSON remain stable data and must not be translated.
pub fn flex_attached_translation_field_eligible(definition: &FieldDefinition) -> bool {
    definition.is_active
        && definition.is_localized
        && matches!(definition.field_type, FieldType::Text | FieldType::Textarea)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedTranslationLeafSnapshot {
    pub leaf: FlexAttachedTranslationLeaf,
    pub required: bool,
    pub source_value: String,
    pub target_value: Option<String>,
}

impl FlexAttachedTranslationLeafSnapshot {
    pub fn validate(&self) -> FlexAttachedTranslationResult<()> {
        self.leaf.validate()?;
        validate_nonblank(&self.source_value, "source_value")?;
        if self
            .target_value
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(FlexAttachedTranslationError::Invalid(
                "Flex attached translation target values must be nonblank or null".to_string(),
            ));
        }
        Ok(())
    }
}

/// Exact source/target view for one canonical donor entity.
///
/// The canonical donor owns existence and lifecycle. Flex owns the durable attached
/// `resource_revision` plus exact localized extension values; the donor adapter composes
/// those sources without deriving a second resource revision. Source/target revisions stay
/// exact-locale content revisions used for leaf-level CAS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedTranslationExactLocaleSnapshot {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub is_active: bool,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    /// Only exact-source fields that are explicitly eligible are exposed.
    pub leaves: Vec<FlexAttachedTranslationLeafSnapshot>,
}

impl FlexAttachedTranslationExactLocaleSnapshot {
    pub fn validate(&self) -> FlexAttachedTranslationResult<()> {
        validate_flex_attached_translation_entity_type(&self.entity_type)?;
        validate_flex_attached_translation_locale_pair(&self.source_locale, &self.target_locale)?;
        validate_nonblank(&self.resource_revision, "resource_revision")?;
        validate_nonblank(&self.source_revision, "source_revision")?;
        if let Some(revision) = &self.target_revision {
            validate_nonblank(revision, "target_revision")?;
        }

        let mut locales = BTreeSet::new();
        for locale in &self.exact_locales {
            validate_authoring_locale(locale, "exact_locales")?;
            if !locales.insert(locale) {
                return Err(FlexAttachedTranslationError::OwnerInvariant(
                    "Flex attached translation exact locale inventory contains duplicates"
                        .to_string(),
                ));
            }
        }

        let mut leaves = BTreeSet::new();
        for leaf in &self.leaves {
            leaf.validate()?;
            if !leaves.insert(leaf.leaf.clone()) {
                return Err(FlexAttachedTranslationError::OwnerInvariant(
                    "Flex attached translation snapshot contains duplicate leaves".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedTranslationResourcePage {
    pub resources: Vec<FlexAttachedTranslationExactLocaleSnapshot>,
    pub next_after: Option<Uuid>,
}

/// Dynamic aggregate progress for one donor and exact locale pair.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedTranslationExactProgress {
    pub required_units: u64,
    pub exact_required_units: u64,
    pub optional_units: u64,
    pub exact_optional_units: u64,
    pub resources: u64,
    pub complete_resources: u64,
}

impl FlexAttachedTranslationExactProgress {
    pub fn validate(&self) -> FlexAttachedTranslationResult<()> {
        if self.exact_required_units > self.required_units {
            return Err(FlexAttachedTranslationError::OwnerInvariant(
                "Flex attached exact required progress exceeds required units".to_string(),
            ));
        }
        if self.exact_optional_units > self.optional_units {
            return Err(FlexAttachedTranslationError::OwnerInvariant(
                "Flex attached exact optional progress exceeds optional units".to_string(),
            ));
        }
        if self.complete_resources > self.resources {
            return Err(FlexAttachedTranslationError::OwnerInvariant(
                "Flex attached complete resources exceed inventory resources".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedTranslationTargetValue {
    pub leaf: FlexAttachedTranslationLeaf,
    /// `None` removes the exact target-locale value for an optional leaf. Requiredness and
    /// complete-leaf-set admission are enforced against the live donor/Flex snapshot.
    pub value: Option<String>,
}

/// Durable workflow evidence that the donor owner must admit atomically with the write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedTranslationOperationContext {
    pub idempotency_key: String,
    pub proposal_id: String,
    pub approval_receipt_id: String,
    pub request_fingerprint: String,
}

impl FlexAttachedTranslationOperationContext {
    pub fn validate(&self) -> FlexAttachedTranslationResult<()> {
        validate_nonblank(&self.idempotency_key, "idempotency_key")?;
        validate_nonblank(&self.proposal_id, "proposal_id")?;
        validate_nonblank(&self.approval_receipt_id, "approval_receipt_id")?;
        validate_nonblank(&self.request_fingerprint, "request_fingerprint")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlexAttachedTranslationExactLocaleApply {
    pub operation: FlexAttachedTranslationOperationContext,
    pub source_locale: String,
    pub target_locale: String,
    /// Complete target values for the exact source-visible leaf set returned by the owner.
    pub target_values: Vec<FlexAttachedTranslationTargetValue>,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

impl FlexAttachedTranslationExactLocaleApply {
    /// Validate immutable request identity before the owner opens a durable operation lease.
    pub fn validate_admission(&self) -> FlexAttachedTranslationResult<()> {
        self.operation.validate()?;
        validate_flex_attached_translation_locale_pair(&self.source_locale, &self.target_locale)?;
        validate_nonblank(&self.expected_resource_revision, "expected_resource_revision")?;
        validate_nonblank(&self.expected_source_revision, "expected_source_revision")?;
        if let Some(revision) = &self.expected_target_revision {
            validate_nonblank(revision, "expected_target_revision")?;
        }
        Ok(())
    }

    pub fn validate(&self) -> FlexAttachedTranslationResult<()> {
        self.validate_admission()?;
        if self.target_values.is_empty() {
            return Err(FlexAttachedTranslationError::Invalid(
                "Flex attached translation apply must contain at least one target value"
                    .to_string(),
            ));
        }

        let mut leaves = BTreeSet::new();
        for target in &self.target_values {
            target.leaf.validate()?;
            if !leaves.insert(target.leaf.clone()) {
                return Err(FlexAttachedTranslationError::Invalid(
                    "Flex attached translation apply contains a duplicate leaf".to_string(),
                ));
            }
            if target
                .value
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
            {
                return Err(FlexAttachedTranslationError::Invalid(
                    "Flex attached translation target values must be nonblank or null".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedTranslationExactLocaleApplyReceipt {
    pub operation_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target_values: Vec<FlexAttachedTranslationTargetValue>,
}

#[derive(Debug)]
pub enum FlexAttachedTranslationError {
    Invalid(String),
    EntityNotFound {
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
    /// Preserve neutral port retry/conflict semantics for durable idempotency admission.
    Operation(PortError),
    Storage(String),
    OwnerInvariant(String),
}

impl fmt::Display for FlexAttachedTranslationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::EntityNotFound {
                entity_type,
                entity_id,
            } => write!(formatter, "Flex attached donor not found: {entity_type}/{entity_id}"),
            Self::SourceLocaleNotFound {
                entity_type,
                entity_id,
                locale,
            } => write!(
                formatter,
                "Flex attached source locale not found: {locale} for {entity_type}/{entity_id}"
            ),
            Self::RevisionConflict { revision } => write!(
                formatter,
                "Flex attached translation {revision} revision conflict"
            ),
            Self::Operation(error) => {
                write!(formatter, "Flex attached translation operation failed: {error}")
            }
            Self::Storage(message) => {
                write!(formatter, "Flex attached translation storage error: {message}")
            }
            Self::OwnerInvariant(message) => write!(
                formatter,
                "Flex attached translation owner invariant failed: {message}"
            ),
        }
    }
}

impl std::error::Error for FlexAttachedTranslationError {}

pub type FlexAttachedTranslationResult<T> = Result<T, FlexAttachedTranslationError>;

pub fn validate_flex_attached_translation_resource_page(
    limit: u16,
) -> FlexAttachedTranslationResult<()> {
    if limit == 0 || limit > MAX_FLEX_ATTACHED_TRANSLATION_RESOURCE_PAGE {
        return Err(FlexAttachedTranslationError::Invalid(format!(
            "Flex attached translation resource page size must be between 1 and {MAX_FLEX_ATTACHED_TRANSLATION_RESOURCE_PAGE}"
        )));
    }
    Ok(())
}

pub fn validate_flex_attached_translation_entity_type(
    entity_type: &str,
) -> FlexAttachedTranslationResult<()> {
    if normalize_flex_entity_type(entity_type).as_deref() != Some(entity_type) {
        return Err(FlexAttachedTranslationError::Invalid(format!(
            "Flex attached translation entity_type must be a normalized Flex donor identifier: {entity_type}"
        )));
    }
    Ok(())
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

fn validate_authoring_locale(locale: &str, field: &str) -> FlexAttachedTranslationResult<()> {
    if locale == LEGACY_UNDETERMINED_LOCALE || normalize_locale_tag(locale).as_deref() != Some(locale)
    {
        return Err(FlexAttachedTranslationError::Invalid(format!(
            "Flex attached translation {field} must be a normalized authoring locale other than `und`"
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

/// Donor-scoped owner contract for attached Flex translation.
///
/// One implementation represents exactly one canonical donor entity type. The adapter composes
/// donor-owned inventory/existence/lifecycle with Flex-owned localized storage and durable
/// `resource_revision`. Donor aggregate revisions may serialize owner mutations, but they are not a
/// second Translation resource revision source.
#[async_trait]
pub trait FlexAttachedTranslationOwnerPort: Send + Sync {
    /// Stable normalized donor identifier (for example `taxonomy.category`).
    fn entity_type(&self) -> &str;

    async fn list_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<Uuid>,
        limit: u16,
    ) -> FlexAttachedTranslationResult<FlexAttachedTranslationResourcePage>;

    async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        entity_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleSnapshot>;

    async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        entity_id: Uuid,
        request: FlexAttachedTranslationExactLocaleApply,
    ) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleApplyReceipt>;
}

/// Separate donor-scoped read model for dynamic aggregate progress.
#[async_trait]
pub trait FlexAttachedTranslationProgressOwnerPort: Send + Sync {
    fn entity_type(&self) -> &str;

    async fn read_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactProgress>;
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rustok_core::field_schema::{FieldDefinition, FieldType};

    use super::{
        FlexAttachedTranslationError, FlexAttachedTranslationExactLocaleApply,
        FlexAttachedTranslationLeaf, FlexAttachedTranslationOperationContext,
        FlexAttachedTranslationTargetValue, flex_attached_translation_field_eligible,
        validate_flex_attached_translation_entity_type,
        validate_flex_attached_translation_locale_pair,
    };

    fn definition(field_type: FieldType, is_localized: bool, is_active: bool) -> FieldDefinition {
        FieldDefinition {
            field_key: "body".to_string(),
            field_type,
            label: HashMap::from([("en".to_string(), "Body".to_string())]),
            description: None,
            is_localized,
            is_required: false,
            default_value: None,
            validation: None,
            position: 0,
            is_active,
        }
    }

    fn operation() -> FlexAttachedTranslationOperationContext {
        FlexAttachedTranslationOperationContext {
            idempotency_key: "idem-1".to_string(),
            proposal_id: "proposal-1".to_string(),
            approval_receipt_id: "approval-1".to_string(),
            request_fingerprint: "sha256:request".to_string(),
        }
    }

    #[test]
    fn only_active_localized_text_copy_is_translation_eligible() {
        assert!(flex_attached_translation_field_eligible(&definition(
            FieldType::Text,
            true,
            true
        )));
        assert!(flex_attached_translation_field_eligible(&definition(
            FieldType::Textarea,
            true,
            true
        )));
        assert!(!flex_attached_translation_field_eligible(&definition(
            FieldType::Select,
            true,
            true
        )));
        assert!(!flex_attached_translation_field_eligible(&definition(
            FieldType::Text,
            false,
            true
        )));
        assert!(!flex_attached_translation_field_eligible(&definition(
            FieldType::Text,
            true,
            false
        )));
    }

    #[test]
    fn donor_and_locale_identity_fail_closed() {
        assert!(validate_flex_attached_translation_entity_type("taxonomy.category").is_ok());
        assert!(validate_flex_attached_translation_entity_type(" Taxonomy.Category ").is_err());
        assert!(validate_flex_attached_translation_locale_pair("en", "ru").is_ok());
        assert!(validate_flex_attached_translation_locale_pair("EN", "ru").is_err());
        assert!(validate_flex_attached_translation_locale_pair("und", "ru").is_err());
        assert!(validate_flex_attached_translation_locale_pair("en", "en").is_err());
    }

    #[test]
    fn apply_requires_unique_nonblank_exact_target_values() {
        let valid = FlexAttachedTranslationExactLocaleApply {
            operation: operation(),
            source_locale: "en".to_string(),
            target_locale: "ru".to_string(),
            target_values: vec![FlexAttachedTranslationTargetValue {
                leaf: FlexAttachedTranslationLeaf {
                    field_key: "body".to_string(),
                },
                value: Some("Текст".to_string()),
            }],
            expected_resource_revision: "resource:1".to_string(),
            expected_source_revision: "source:1".to_string(),
            expected_target_revision: None,
        };
        assert!(valid.validate().is_ok());

        let mut duplicate = valid.clone();
        duplicate.target_values.push(duplicate.target_values[0].clone());
        assert!(matches!(
            duplicate.validate(),
            Err(FlexAttachedTranslationError::Invalid(_))
        ));

        let mut blank = valid;
        blank.target_values[0].value = Some("   ".to_string());
        assert!(matches!(
            blank.validate(),
            Err(FlexAttachedTranslationError::Invalid(_))
        ));
    }
}
