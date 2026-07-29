use std::collections::{BTreeMap, BTreeSet};
#[cfg(feature = "runtime")]
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, TenantLocale};
use rustok_translation_targets::{
    TranslationDataClassification, TranslationStrategy, TranslationValueProfile,
};
use serde::{Deserialize, Serialize};

pub const MAX_MACHINE_TRANSLATION_BATCH_UNITS: usize = 100;
pub const MAX_MACHINE_TRANSLATION_BATCH_CHARACTERS: usize = 200_000;
pub const MAX_MACHINE_TRANSLATION_GLOSSARY_TERMS: usize = 500;
pub const MAX_MACHINE_TRANSLATION_MEMORY_SUGGESTIONS_PER_UNIT: usize = 5;
pub const MAX_MACHINE_TRANSLATION_PROTECTED_TOKENS_PER_UNIT: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationProviderDescriptor {
    pub slug: String,
    pub display_name: String,
    pub policy_digest: String,
    pub supported_profiles: Vec<TranslationValueProfile>,
    pub supported_classifications: Vec<TranslationDataClassification>,
    pub max_batch_units: u16,
    pub max_batch_characters: u32,
    pub review_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineTranslationProviderState {
    Available,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationProviderHealth {
    pub state: MachineTranslationProviderState,
    pub reason_code: Option<String>,
    pub retry_after_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationResourceContext {
    pub owner_slug: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub subresource_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationUnit {
    pub unit_id: String,
    pub field_key: String,
    pub source_value: String,
    pub source_hash: String,
    pub source_revision: String,
    pub profile: TranslationValueProfile,
    pub strategy: TranslationStrategy,
    pub classification: TranslationDataClassification,
    pub ai_export_allowed: bool,
    pub max_characters: Option<u32>,
    pub preserves_whitespace: bool,
    pub protected_tokens: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationGlossaryTerm {
    pub concept_id: String,
    pub source_term: String,
    pub preferred_target_term: Option<String>,
    pub allowed_target_terms: Vec<String>,
    pub forbidden_target_terms: Vec<String>,
    pub do_not_translate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationMemorySuggestion {
    pub unit_id: String,
    pub entry_id: String,
    pub source_value: String,
    pub target_value: String,
    pub score_basis_points: u16,
    pub source_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationBatchRequest {
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub resource: MachineTranslationResourceContext,
    pub units: Vec<MachineTranslationUnit>,
    pub glossary_revision: Option<String>,
    pub glossary_digest: Option<String>,
    pub glossary_terms: Vec<MachineTranslationGlossaryTerm>,
    pub memory_digest: Option<String>,
    pub memory_suggestions: Vec<MachineTranslationMemorySuggestion>,
    pub tone: Option<String>,
    pub domain: Option<String>,
    pub style: Option<String>,
    pub adapter_policy_digest: String,
    pub evidence: BTreeMap<String, String>,
}

impl MachineTranslationBatchRequest {
    pub fn validate(&self, context: &PortContext) -> Result<(), PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if self.source_locale == self.target_locale {
            return Err(PortError::validation(
                "translation.machine.locale_pair_invalid",
                "machine translation source and target locale must differ",
            ));
        }
        if self.units.is_empty() || self.units.len() > MAX_MACHINE_TRANSLATION_BATCH_UNITS {
            return Err(PortError::validation(
                "translation.machine.batch_size_invalid",
                format!(
                    "machine translation batch must contain 1..={MAX_MACHINE_TRANSLATION_BATCH_UNITS} units"
                ),
            ));
        }
        require_identity("owner_slug", &self.resource.owner_slug)?;
        require_identity("resource_kind", &self.resource.resource_kind)?;
        require_identity("resource_id", &self.resource.resource_id)?;
        require_digest("adapter_policy_digest", &self.adapter_policy_digest)?;
        validate_optional_digest("glossary_digest", self.glossary_digest.as_deref())?;
        validate_optional_digest("memory_digest", self.memory_digest.as_deref())?;
        if self.glossary_revision.is_some() != self.glossary_digest.is_some() {
            return Err(PortError::validation(
                "translation.machine.glossary_binding_invalid",
                "glossary revision and digest must be supplied together",
            ));
        }
        if self.glossary_terms.len() > MAX_MACHINE_TRANSLATION_GLOSSARY_TERMS
            || self.glossary_terms.iter().any(|term| {
                term.concept_id.trim().is_empty()
                    || term.concept_id.len() > 256
                    || term.source_term.trim().is_empty()
                    || term.source_term.len() > 512
                    || term
                        .preferred_target_term
                        .as_ref()
                        .is_some_and(|value| value.trim().is_empty() || value.len() > 512)
                    || term.allowed_target_terms.len() > 32
                    || term.forbidden_target_terms.len() > 32
                    || term
                        .allowed_target_terms
                        .iter()
                        .chain(&term.forbidden_target_terms)
                        .any(|value| value.trim().is_empty() || value.len() > 512)
            })
        {
            return Err(PortError::validation(
                "translation.machine.glossary_terms_invalid",
                "machine translation glossary context exceeds its bounded term contract",
            ));
        }
        if self.evidence.len() > 32
            || self.evidence.iter().any(|(key, value)| {
                key.trim().is_empty()
                    || key.len() > 64
                    || value.trim().is_empty()
                    || value.len() > 256
            })
            || [&self.tone, &self.domain, &self.style]
                .into_iter()
                .flatten()
                .any(|value| value.trim().is_empty() || value.len() > 256)
        {
            return Err(PortError::validation(
                "translation.machine.context_invalid",
                "machine translation style and evidence context must remain bounded",
            ));
        }

        let mut unit_ids = BTreeSet::new();
        let total_characters = self.units.iter().try_fold(0usize, |total, unit| {
            validate_unit(unit)?;
            if !unit_ids.insert(unit.unit_id.as_str()) {
                return Err(PortError::validation(
                    "translation.machine.unit_duplicate",
                    "machine translation batch contains a duplicate unit_id",
                ));
            }
            Ok(total.saturating_add(unit.source_value.chars().count()))
        })?;
        if total_characters > MAX_MACHINE_TRANSLATION_BATCH_CHARACTERS {
            return Err(PortError::validation(
                "translation.machine.batch_characters_exceeded",
                format!(
                    "machine translation batch exceeds {MAX_MACHINE_TRANSLATION_BATCH_CHARACTERS} characters"
                ),
            ));
        }

        if self.memory_suggestions.len()
            > self
                .units
                .len()
                .saturating_mul(MAX_MACHINE_TRANSLATION_MEMORY_SUGGESTIONS_PER_UNIT)
            || self.memory_suggestions.iter().any(|suggestion| {
                !unit_ids.contains(suggestion.unit_id.as_str())
                    || suggestion.entry_id.trim().is_empty()
                    || suggestion.entry_id.len() > 256
                    || suggestion.source_value.len() > 20_000
                    || suggestion.target_value.len() > 20_000
                    || suggestion.score_basis_points > 10_000
                    || require_digest("memory_source_hash", &suggestion.source_hash).is_err()
            })
        {
            return Err(PortError::validation(
                "translation.machine.memory_suggestion_invalid",
                "memory suggestions must reference a batch unit and use a 0..=10000 score",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationDiagnostic {
    pub code: String,
    pub blocking: bool,
    pub unit_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationUnitResult {
    pub unit_id: String,
    pub translated_value: String,
    pub protected_tokens: Vec<String>,
    pub diagnostics: Vec<MachineTranslationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost_minor_units: u64,
    pub currency_code: String,
    pub price_snapshot_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationAttemptEvidence {
    pub attempt: u16,
    pub provider_profile_id: String,
    pub provider_slug: String,
    pub model: String,
    pub fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationExecutionEvidence {
    pub execution_id: String,
    pub request_digest: String,
    pub prompt_policy_digest: String,
    pub attempts: Vec<MachineTranslationAttemptEvidence>,
    pub usage: MachineTranslationUsage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationBatchResult {
    pub provider_slug: String,
    pub units: Vec<MachineTranslationUnitResult>,
    pub execution: MachineTranslationExecutionEvidence,
    pub review_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineTranslationExecutionStatus {
    NotRegistered,
    Queued,
    Running,
    CancellationRequested,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTranslationExecutionStatusEvidence {
    pub execution_id: Option<String>,
    pub status: MachineTranslationExecutionStatus,
}

#[async_trait]
pub trait MachineTranslationPort: Send + Sync {
    fn descriptor(&self) -> &MachineTranslationProviderDescriptor;

    async fn health(
        &self,
        context: PortContext,
    ) -> Result<MachineTranslationProviderHealth, PortError>;

    async fn translate_batch(
        &self,
        context: PortContext,
        request: MachineTranslationBatchRequest,
    ) -> Result<MachineTranslationBatchResult, PortError>;

    async fn execution_status(
        &self,
        context: PortContext,
        execution_idempotency_key: String,
    ) -> Result<MachineTranslationExecutionStatusEvidence, PortError>;

    async fn recover_batch(
        &self,
        context: PortContext,
        execution_idempotency_key: String,
        request: MachineTranslationBatchRequest,
    ) -> Result<Option<MachineTranslationBatchResult>, PortError>;

    async fn cancel_execution(
        &self,
        context: PortContext,
        execution_idempotency_key: String,
    ) -> Result<MachineTranslationExecutionStatusEvidence, PortError>;
}

/// Deployment-composed factory for the optional machine-translation provider.
///
/// Translation owns this neutral lazy boundary so the host can transfer the
/// factory through runtime extensions before a database-backed host context
/// exists. Concrete AI/provider crates remain outside this owner crate.
#[cfg(feature = "runtime")]
pub trait MachineTranslationPortFactory: Send + Sync {
    fn create(
        &self,
        context: &rustok_api::HostRuntimeContext,
    ) -> Result<Option<Arc<dyn MachineTranslationPort>>, PortError>;
}

#[cfg(feature = "runtime")]
#[derive(Clone)]
pub struct SharedMachineTranslationPortFactory(pub Arc<dyn MachineTranslationPortFactory>);

#[cfg(feature = "runtime")]
pub fn machine_translation_port_from_context(
    context: &rustok_api::HostRuntimeContext,
) -> Result<Option<Arc<dyn MachineTranslationPort>>, PortError> {
    let Some(factory) = context.shared_get::<SharedMachineTranslationPortFactory>() else {
        return Ok(None);
    };
    factory.0.create(context)
}

fn validate_unit(unit: &MachineTranslationUnit) -> Result<(), PortError> {
    require_identity("unit_id", &unit.unit_id)?;
    require_identity("field_key", &unit.field_key)?;
    require_identity("source_revision", &unit.source_revision)?;
    require_digest("source_hash", &unit.source_hash)?;
    if unit.source_value.is_empty() {
        return Err(PortError::validation(
            "translation.machine.source_empty",
            "machine translation source value must not be empty",
        ));
    }
    if !unit.ai_export_allowed {
        return Err(PortError::forbidden(
            "translation.machine.ai_export_forbidden",
            "the owner does not allow this field to be exported to AI",
        ));
    }
    if !matches!(
        unit.strategy,
        TranslationStrategy::Translate | TranslationStrategy::TranslateWithPlaceholders
    ) {
        return Err(PortError::validation(
            "translation.machine.strategy_unsupported",
            "the field translation strategy is not supported by machine translation",
        ));
    }
    if matches!(
        unit.classification,
        TranslationDataClassification::Secret | TranslationDataClassification::ImmutableTransaction
    ) {
        return Err(PortError::forbidden(
            "translation.machine.classification_forbidden",
            "the field data classification forbids AI translation",
        ));
    }
    let mut protected_tokens = BTreeSet::new();
    if unit.protected_tokens.len() > MAX_MACHINE_TRANSLATION_PROTECTED_TOKENS_PER_UNIT
        || unit.protected_tokens.iter().any(|token| {
            token.is_empty()
                || token.len() > 256
                || !protected_tokens.insert(token.as_str())
                || !unit.source_value.contains(token)
        })
    {
        return Err(PortError::validation(
            "translation.machine.protected_tokens_invalid",
            "protected tokens must be unique, non-empty, and present in the source value",
        ));
    }
    Ok(())
}

fn require_identity(field: &'static str, value: &str) -> Result<(), PortError> {
    if value.trim().is_empty() || value.len() > 256 {
        return Err(PortError::validation(
            format!("translation.machine.{field}_invalid"),
            format!("{field} must contain 1..=256 non-whitespace bytes"),
        ));
    }
    Ok(())
}

fn require_digest(field: &'static str, value: &str) -> Result<(), PortError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(PortError::validation(
            format!("translation.machine.{field}_invalid"),
            format!("{field} must be a SHA-256 hex digest"),
        ));
    }
    Ok(())
}

fn validate_optional_digest(field: &'static str, value: Option<&str>) -> Result<(), PortError> {
    match value {
        Some(value) => require_digest(field, value),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_api::{PortActor, PortErrorKind};

    use super::*;

    fn context() -> PortContext {
        PortContext::new("tenant-a", PortActor::service("service-a"), "en", "corr-a")
            .with_idempotency_key("idem-a")
            .with_deadline(Duration::from_secs(5))
    }

    fn request() -> MachineTranslationBatchRequest {
        MachineTranslationBatchRequest {
            source_locale: TenantLocale::new("en").unwrap(),
            target_locale: TenantLocale::new("de").unwrap(),
            resource: MachineTranslationResourceContext {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                resource_id: "asset-a".to_string(),
                subresource_id: None,
            },
            units: vec![MachineTranslationUnit {
                unit_id: "unit-a".to_string(),
                field_key: "alt_text".to_string(),
                source_value: "Hello {name}".to_string(),
                source_hash: "a".repeat(64),
                source_revision: "revision-a".to_string(),
                profile: TranslationValueProfile::TemplateText,
                strategy: TranslationStrategy::TranslateWithPlaceholders,
                classification: TranslationDataClassification::TenantPrivate,
                ai_export_allowed: true,
                max_characters: Some(200),
                preserves_whitespace: false,
                protected_tokens: vec!["{name}".to_string()],
            }],
            glossary_revision: None,
            glossary_digest: None,
            glossary_terms: Vec::new(),
            memory_digest: None,
            memory_suggestions: Vec::new(),
            tone: None,
            domain: None,
            style: None,
            adapter_policy_digest: "b".repeat(64),
            evidence: BTreeMap::new(),
        }
    }

    #[test]
    fn batch_requires_write_semantics() {
        let context = PortContext::new("tenant-a", PortActor::service("service-a"), "en", "corr-a");
        assert_eq!(
            request().validate(&context).unwrap_err().kind,
            PortErrorKind::Validation
        );
    }

    #[test]
    fn batch_rejects_unsafe_classification() {
        let mut request = request();
        request.units[0].classification = TranslationDataClassification::Secret;
        assert_eq!(
            request.validate(&context()).unwrap_err().code,
            "translation.machine.classification_forbidden"
        );
    }

    #[test]
    fn batch_rejects_missing_protected_token() {
        let mut request = request();
        request.units[0].protected_tokens = vec!["{missing}".to_string()];
        assert_eq!(
            request.validate(&context()).unwrap_err().code,
            "translation.machine.protected_tokens_invalid"
        );
    }
}
