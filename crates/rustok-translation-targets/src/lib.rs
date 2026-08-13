use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, TenantLocale};
use rustok_core::ModuleRuntimeExtensions;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_IDENTITY_LENGTH: usize = 191;
const MAX_REVISION_LENGTH: usize = 256;
pub const MAX_TRANSLATION_RESOURCE_PAGE_SIZE: u16 = 200;

pub mod provider_support;

macro_rules! string_identity {
    ($name:ident, $label:literal, $max:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, TranslationTargetContractError> {
                let value = value.into();
                let normalized = value.trim();
                if normalized.is_empty() || normalized.len() > $max {
                    return Err(TranslationTargetContractError::InvalidIdentity {
                        field: $label,
                        max: $max,
                    });
                }
                Ok(Self(normalized.to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl TryFrom<String> for $name {
            type Error = TranslationTargetContractError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

string_identity!(OwnerSlug, "owner_slug", MAX_IDENTITY_LENGTH);
string_identity!(ResourceKind, "resource_kind", MAX_IDENTITY_LENGTH);
string_identity!(ResourceId, "resource_id", MAX_IDENTITY_LENGTH);
string_identity!(FieldKey, "field_key", MAX_IDENTITY_LENGTH);
string_identity!(OpaqueRevision, "revision", MAX_REVISION_LENGTH);
string_identity!(OpaqueCursor, "cursor", MAX_REVISION_LENGTH);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TranslationTargetContractError {
    #[error("{field} must contain 1..={max} non-whitespace bytes")]
    InvalidIdentity { field: &'static str, max: usize },
    #[error("provider display_name must not be empty")]
    EmptyDisplayName,
    #[error("provider must declare at least one capability")]
    MissingCapability,
    #[error("provider capability `{0}` requires `{1}`")]
    InvalidCapabilityCombination(&'static str, &'static str),
    #[error("resource snapshot source and target locale must differ")]
    EqualSourceAndTargetLocale,
    #[error("resource page limit must be between 1 and {MAX_TRANSLATION_RESOURCE_PAGE_SIZE}")]
    InvalidPageLimit,
    #[error("resource snapshot source locale is not present in exact_locales")]
    SourceLocaleIsNotExact,
    #[error("resource snapshot contains a duplicate exact locale")]
    DuplicateExactLocale,
    #[error("resource snapshot contains a duplicate field key")]
    DuplicateFieldKey,
    #[error("translation patch must contain at least one field")]
    EmptyPatch,
    #[error("translation patch contains a duplicate field key")]
    DuplicatePatchFieldKey,
    #[error("translation patch {0} must not be empty")]
    EmptyReceiptIdentity(&'static str),
    #[error("AI-exportable field `{0}` has a forbidden data classification")]
    UnsafeAiExport(FieldKey),
    #[error("translation progress exact required units exceed required units")]
    ExactRequiredUnitsOverflow,
    #[error("translation progress exact optional units exceed optional units")]
    ExactOptionalUnitsOverflow,
    #[error("translation progress complete resources exceed resources")]
    CompleteResourcesOverflow,
    #[error("translation field contains an empty protected token")]
    EmptyProtectedToken,
    #[error("translation field contains a duplicate protected token")]
    DuplicateProtectedToken,
    #[error("translation field protected token is absent from the source value")]
    ProtectedTokenMissingFromSource,
    #[error("translation patch validation issue code must not be empty")]
    EmptyPatchIssueCode,
    #[error("translation patch validation issue message must not be empty")]
    EmptyPatchIssueMessage,
    #[error("translation patch validation acceptance does not match issue severities")]
    PatchValidationAcceptanceMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationTargetCapability {
    ListResources,
    ReadExactResource,
    AggregateProgress,
    ValidatePatch,
    ApplyPatch,
    ChangeCursor,
    Export,
    Import,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationValueProfile {
    PlainText,
    SeoText,
    TemplateText,
    Richtext,
    PageBuilderText,
    LocalizedScalar,
    Slug,
    Identifier,
    Url,
    Email,
    Secret,
    Code,
    EnumKey,
    ImmutableTransactionSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationDataClassification {
    Public,
    TenantPrivate,
    Personal,
    Sensitive,
    Secret,
    ImmutableTransaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationStrategy {
    Translate,
    TranslateWithPlaceholders,
    TransliterateWithReview,
    ManualOnly,
    Excluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationResourceLifecycle {
    Active,
    Archived,
    Deleted,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationTargetProviderDescriptor {
    pub owner_slug: OwnerSlug,
    pub resource_kind: ResourceKind,
    pub display_name: String,
    pub capabilities: BTreeSet<TranslationTargetCapability>,
    pub read_permission_floor: BTreeSet<String>,
    pub apply_permission_floor: BTreeSet<String>,
}

impl TranslationTargetProviderDescriptor {
    pub fn validate(&self) -> Result<(), TranslationTargetContractError> {
        if self.display_name.trim().is_empty() {
            return Err(TranslationTargetContractError::EmptyDisplayName);
        }
        if self.capabilities.is_empty() {
            return Err(TranslationTargetContractError::MissingCapability);
        }
        if self
            .capabilities
            .contains(&TranslationTargetCapability::ApplyPatch)
            && !self
                .capabilities
                .contains(&TranslationTargetCapability::ValidatePatch)
        {
            return Err(
                TranslationTargetContractError::InvalidCapabilityCombination(
                    "apply_patch",
                    "validate_patch",
                ),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TranslationResourceIdentity {
    pub owner_slug: OwnerSlug,
    pub resource_kind: ResourceKind,
    pub resource_id: ResourceId,
    pub subresource_id: Option<ResourceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationFieldDescriptor {
    pub key: FieldKey,
    pub profile: TranslationValueProfile,
    pub strategy: TranslationStrategy,
    pub classification: TranslationDataClassification,
    pub required: bool,
    pub ai_export_allowed: bool,
    pub max_characters: Option<u32>,
    pub preserves_whitespace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationFieldSnapshot {
    pub descriptor: TranslationFieldDescriptor,
    pub source_value: String,
    pub exact_target_value: Option<String>,
    pub source_hash: String,
    pub protected_tokens: Vec<String>,
}

/// Returns whether two protected-token ledgers contain exactly the same unique
/// tokens. Ledger ordering is intentionally not semantic, while duplicate
/// evidence is always invalid.
pub fn protected_token_ledger_matches(expected: &[String], actual: &[String]) -> bool {
    let expected_tokens = expected.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let actual_tokens = actual.iter().map(String::as_str).collect::<BTreeSet<_>>();
    expected_tokens.len() == expected.len()
        && actual_tokens.len() == actual.len()
        && expected_tokens == actual_tokens
}

/// Returns whether every owner-declared protected token occurs exactly as many
/// times in a translated value as it does in its source value.
pub fn protected_token_multiplicities_match(
    source_value: &str,
    translated_value: &str,
    protected_tokens: &[String],
) -> bool {
    protected_tokens.iter().all(|token| {
        token_occurrences(source_value, token) == token_occurrences(translated_value, token)
    })
}

/// Returns whether leading/trailing whitespace and every line-break sequence
/// are preserved between an owner source value and its translation.
pub fn whitespace_shape_matches(source_value: &str, translated_value: &str) -> bool {
    whitespace_shape(source_value) == whitespace_shape(translated_value)
}

fn token_occurrences(value: &str, token: &str) -> usize {
    value.match_indices(token).count()
}

fn whitespace_shape(value: &str) -> (String, String, Vec<String>) {
    let leading = value
        .chars()
        .take_while(|character| character.is_whitespace())
        .collect();
    let trailing = value
        .chars()
        .rev()
        .take_while(|character| character.is_whitespace())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let line_breaks = value
        .split_inclusive('\n')
        .filter_map(|line| {
            line.strip_suffix("\r\n")
                .map(|_| "\r\n".to_string())
                .or_else(|| line.strip_suffix('\n').map(|_| "\n".to_string()))
        })
        .collect();
    (leading, trailing, line_breaks)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationResourceSummary {
    pub identity: TranslationResourceIdentity,
    pub display_label: String,
    pub lifecycle: TranslationResourceLifecycle,
    pub resource_revision: OpaqueRevision,
    pub exact_locales: Vec<TenantLocale>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationResourceSnapshot {
    pub summary: TranslationResourceSummary,
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub rendered_fallback_locale: Option<TenantLocale>,
    pub source_revision: OpaqueRevision,
    pub target_revision: Option<OpaqueRevision>,
    pub fields: Vec<TranslationFieldSnapshot>,
}

impl TranslationResourceSnapshot {
    pub fn validate(&self) -> Result<(), TranslationTargetContractError> {
        if self.source_locale == self.target_locale {
            return Err(TranslationTargetContractError::EqualSourceAndTargetLocale);
        }

        let mut locales = BTreeSet::new();
        for locale in &self.summary.exact_locales {
            if !locales.insert(locale.as_str()) {
                return Err(TranslationTargetContractError::DuplicateExactLocale);
            }
        }
        if !locales.contains(self.source_locale.as_str()) {
            return Err(TranslationTargetContractError::SourceLocaleIsNotExact);
        }

        let mut fields = BTreeSet::new();
        for field in &self.fields {
            if !fields.insert(field.descriptor.key.as_str()) {
                return Err(TranslationTargetContractError::DuplicateFieldKey);
            }
            if field.descriptor.ai_export_allowed
                && matches!(
                    field.descriptor.classification,
                    TranslationDataClassification::Secret
                        | TranslationDataClassification::ImmutableTransaction
                )
            {
                return Err(TranslationTargetContractError::UnsafeAiExport(
                    field.descriptor.key.clone(),
                ));
            }
            let mut protected_tokens = BTreeSet::new();
            for token in &field.protected_tokens {
                if token.is_empty() {
                    return Err(TranslationTargetContractError::EmptyProtectedToken);
                }
                if !protected_tokens.insert(token.as_str()) {
                    return Err(TranslationTargetContractError::DuplicateProtectedToken);
                }
                if !field.source_value.contains(token) {
                    return Err(TranslationTargetContractError::ProtectedTokenMissingFromSource);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListTranslationResourcesRequest {
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub cursor: Option<OpaqueCursor>,
    pub limit: u16,
}

impl ListTranslationResourcesRequest {
    pub fn validate(&self) -> Result<(), TranslationTargetContractError> {
        if self.source_locale == self.target_locale {
            return Err(TranslationTargetContractError::EqualSourceAndTargetLocale);
        }
        if self.limit == 0 || self.limit > MAX_TRANSLATION_RESOURCE_PAGE_SIZE {
            return Err(TranslationTargetContractError::InvalidPageLimit);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationResourcePage {
    pub resources: Vec<TranslationResourceSummary>,
    pub next_cursor: Option<OpaqueCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationTargetProgressRequest {
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
}

impl TranslationTargetProgressRequest {
    pub fn validate(&self) -> Result<(), TranslationTargetContractError> {
        if self.source_locale == self.target_locale {
            return Err(TranslationTargetContractError::EqualSourceAndTargetLocale);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationTargetProgressFacts {
    pub required_units: u64,
    pub exact_required_units: u64,
    pub optional_units: u64,
    pub exact_optional_units: u64,
    pub resources: u64,
    pub complete_resources: u64,
    pub owner_change_cursor: Option<OpaqueCursor>,
}

impl TranslationTargetProgressFacts {
    pub fn validate(&self) -> Result<(), TranslationTargetContractError> {
        if self.exact_required_units > self.required_units {
            return Err(TranslationTargetContractError::ExactRequiredUnitsOverflow);
        }
        if self.exact_optional_units > self.optional_units {
            return Err(TranslationTargetContractError::ExactOptionalUnitsOverflow);
        }
        if self.complete_resources > self.resources {
            return Err(TranslationTargetContractError::CompleteResourcesOverflow);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationTargetChangesRequest {
    pub after: Option<OpaqueCursor>,
    pub limit: u16,
}

impl TranslationTargetChangesRequest {
    pub fn validate(&self) -> Result<(), TranslationTargetContractError> {
        if self.limit == 0 || self.limit > MAX_TRANSLATION_RESOURCE_PAGE_SIZE {
            return Err(TranslationTargetContractError::InvalidPageLimit);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationTargetChange {
    pub identity: TranslationResourceIdentity,
    pub resource_revision: OpaqueRevision,
    pub lifecycle: TranslationResourceLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationTargetChangePage {
    pub changes: Vec<TranslationTargetChange>,
    pub next_cursor: Option<OpaqueCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationTargetExportPage {
    pub resources: Vec<TranslationResourceSnapshot>,
    pub next_cursor: Option<OpaqueCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationTargetImportRequest {
    pub patches: Vec<TranslationPatchRequest>,
}

impl TranslationTargetImportRequest {
    pub fn validate(&self) -> Result<(), TranslationTargetContractError> {
        if self.patches.is_empty() {
            return Err(TranslationTargetContractError::EmptyPatch);
        }
        self.patches
            .iter()
            .try_for_each(TranslationPatchRequest::validate)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationTargetImportValidation {
    pub patches: Vec<TranslationPatchValidation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadTranslationResourceRequest {
    pub identity: TranslationResourceIdentity,
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationFieldPatch {
    pub key: FieldKey,
    pub value: String,
    pub expected_source_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationPatchRequest {
    pub identity: TranslationResourceIdentity,
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub expected_resource_revision: OpaqueRevision,
    pub expected_source_revision: OpaqueRevision,
    pub expected_target_revision: Option<OpaqueRevision>,
    pub fields: Vec<TranslationFieldPatch>,
    pub proposal_id: String,
    pub approval_receipt_id: String,
}

impl TranslationPatchRequest {
    pub fn validate(&self) -> Result<(), TranslationTargetContractError> {
        if self.source_locale == self.target_locale {
            return Err(TranslationTargetContractError::EqualSourceAndTargetLocale);
        }
        if self.fields.is_empty() {
            return Err(TranslationTargetContractError::EmptyPatch);
        }
        let mut fields = BTreeSet::new();
        for field in &self.fields {
            if !fields.insert(field.key.as_str()) {
                return Err(TranslationTargetContractError::DuplicatePatchFieldKey);
            }
        }
        if self.proposal_id.trim().is_empty() {
            return Err(TranslationTargetContractError::EmptyReceiptIdentity(
                "proposal_id",
            ));
        }
        if self.approval_receipt_id.trim().is_empty() {
            return Err(TranslationTargetContractError::EmptyReceiptIdentity(
                "approval_receipt_id",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationPatchValidation {
    pub accepted: bool,
    pub issues: Vec<TranslationPatchIssue>,
}

impl TranslationPatchValidation {
    pub fn validate(&self) -> Result<(), TranslationTargetContractError> {
        for issue in &self.issues {
            if issue.code.trim().is_empty() {
                return Err(TranslationTargetContractError::EmptyPatchIssueCode);
            }
            if issue.message.trim().is_empty() {
                return Err(TranslationTargetContractError::EmptyPatchIssueMessage);
            }
        }
        let has_error = self
            .issues
            .iter()
            .any(|issue| issue.severity == TranslationPatchIssueSeverity::Error);
        if self.accepted == has_error {
            return Err(TranslationTargetContractError::PatchValidationAcceptanceMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationPatchIssueSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationPatchIssue {
    pub field: Option<FieldKey>,
    pub severity: TranslationPatchIssueSeverity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationApplicationReceipt {
    pub provider_receipt_id: String,
    pub resource_revision: OpaqueRevision,
    pub target_revision: OpaqueRevision,
    pub applied_field_keys: Vec<FieldKey>,
}

#[async_trait]
pub trait TranslationTargetProvider: Send + Sync {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor;

    async fn list_resources(
        &self,
        context: PortContext,
        request: ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError>;

    async fn read_resource(
        &self,
        context: PortContext,
        request: ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError>;

    async fn validate_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError>;

    async fn apply_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError>;

    async fn read_progress(
        &self,
        _context: PortContext,
        _request: TranslationTargetProgressRequest,
    ) -> Result<TranslationTargetProgressFacts, PortError> {
        Err(capability_unavailable("aggregate_progress"))
    }

    async fn read_changes(
        &self,
        _context: PortContext,
        _request: TranslationTargetChangesRequest,
    ) -> Result<TranslationTargetChangePage, PortError> {
        Err(capability_unavailable("change_cursor"))
    }

    async fn export_resources(
        &self,
        _context: PortContext,
        _request: ListTranslationResourcesRequest,
    ) -> Result<TranslationTargetExportPage, PortError> {
        Err(capability_unavailable("export"))
    }

    async fn validate_import(
        &self,
        _context: PortContext,
        _request: TranslationTargetImportRequest,
    ) -> Result<TranslationTargetImportValidation, PortError> {
        Err(capability_unavailable("import"))
    }
}

fn capability_unavailable(capability: &'static str) -> PortError {
    PortError::validation(
        "translation.target_capability_unavailable",
        format!("translation target provider does not implement {capability}"),
    )
}

pub fn validate_translation_read_context(context: &PortContext) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read())
}

pub fn validate_translation_apply_context(context: &PortContext) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::write())
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TranslationTargetRegistryError {
    #[error("invalid translation target provider descriptor: {0}")]
    InvalidDescriptor(#[from] TranslationTargetContractError),
    #[error("translation target provider `{owner_slug}/{resource_kind}` is already registered")]
    DuplicateProvider {
        owner_slug: OwnerSlug,
        resource_kind: ResourceKind,
    },
}

#[derive(Clone, Default)]
pub struct TranslationTargetRegistry {
    providers: BTreeMap<(OwnerSlug, ResourceKind), Arc<dyn TranslationTargetProvider>>,
}

impl TranslationTargetRegistry {
    pub fn register<P>(&mut self, provider: P) -> Result<(), TranslationTargetRegistryError>
    where
        P: TranslationTargetProvider + 'static,
    {
        self.register_arc(Arc::new(provider))
    }

    pub fn register_arc(
        &mut self,
        provider: Arc<dyn TranslationTargetProvider>,
    ) -> Result<(), TranslationTargetRegistryError> {
        let descriptor = provider.descriptor();
        descriptor.validate()?;
        let key = (
            descriptor.owner_slug.clone(),
            descriptor.resource_kind.clone(),
        );
        if self.providers.contains_key(&key) {
            return Err(TranslationTargetRegistryError::DuplicateProvider {
                owner_slug: key.0,
                resource_kind: key.1,
            });
        }
        self.providers.insert(key, provider);
        Ok(())
    }

    pub fn get(
        &self,
        owner_slug: &OwnerSlug,
        resource_kind: &ResourceKind,
    ) -> Option<Arc<dyn TranslationTargetProvider>> {
        self.providers
            .get(&(owner_slug.clone(), resource_kind.clone()))
            .cloned()
    }

    pub fn descriptors(&self) -> Vec<TranslationTargetProviderDescriptor> {
        self.providers
            .values()
            .map(|provider| provider.descriptor())
            .collect()
    }
}

pub fn register_translation_target_provider<P>(
    extensions: &mut ModuleRuntimeExtensions,
    provider: P,
) -> Result<(), TranslationTargetRegistryError>
where
    P: TranslationTargetProvider + 'static,
{
    let registry = extensions.get_or_insert_with::<Arc<TranslationTargetRegistry>, _>(|| {
        Arc::new(TranslationTargetRegistry::default())
    });
    Arc::make_mut(registry).register(provider)
}

pub fn translation_target_registry(
    extensions: &ModuleRuntimeExtensions,
) -> Option<Arc<TranslationTargetRegistry>> {
    extensions.get::<Arc<TranslationTargetRegistry>>().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyProvider;

    impl DummyProvider {
        fn descriptor() -> TranslationTargetProviderDescriptor {
            TranslationTargetProviderDescriptor {
                owner_slug: OwnerSlug::new("content").unwrap(),
                resource_kind: ResourceKind::new("article").unwrap(),
                display_name: "Article".to_string(),
                capabilities: BTreeSet::from([
                    TranslationTargetCapability::ListResources,
                    TranslationTargetCapability::ReadExactResource,
                    TranslationTargetCapability::ValidatePatch,
                    TranslationTargetCapability::ApplyPatch,
                ]),
                read_permission_floor: BTreeSet::from(["content:read".to_string()]),
                apply_permission_floor: BTreeSet::from(["content:update".to_string()]),
            }
        }
    }

    #[async_trait]
    impl TranslationTargetProvider for DummyProvider {
        fn descriptor(&self) -> TranslationTargetProviderDescriptor {
            Self::descriptor()
        }

        async fn list_resources(
            &self,
            _context: PortContext,
            _request: ListTranslationResourcesRequest,
        ) -> Result<TranslationResourcePage, PortError> {
            unreachable!()
        }

        async fn read_resource(
            &self,
            _context: PortContext,
            _request: ReadTranslationResourceRequest,
        ) -> Result<TranslationResourceSnapshot, PortError> {
            unreachable!()
        }

        async fn validate_patch(
            &self,
            _context: PortContext,
            _request: TranslationPatchRequest,
        ) -> Result<TranslationPatchValidation, PortError> {
            unreachable!()
        }

        async fn apply_patch(
            &self,
            _context: PortContext,
            _request: TranslationPatchRequest,
        ) -> Result<TranslationApplicationReceipt, PortError> {
            unreachable!()
        }
    }

    #[test]
    fn registry_rejects_duplicate_owner_resource_kind() {
        let mut registry = TranslationTargetRegistry::default();
        registry.register(DummyProvider).unwrap();
        let error = registry.register(DummyProvider).unwrap_err();
        assert!(matches!(
            error,
            TranslationTargetRegistryError::DuplicateProvider { .. }
        ));
    }

    #[test]
    fn runtime_extension_helper_publishes_registry() {
        let mut extensions = ModuleRuntimeExtensions::default();
        register_translation_target_provider(&mut extensions, DummyProvider).unwrap();
        let registry = translation_target_registry(&extensions).unwrap();
        assert_eq!(registry.descriptors(), vec![DummyProvider::descriptor()]);
    }

    #[test]
    fn snapshot_conformance_separates_exact_locale_and_fallback() {
        let field_key = FieldKey::new("title").unwrap();
        let snapshot = TranslationResourceSnapshot {
            summary: TranslationResourceSummary {
                identity: TranslationResourceIdentity {
                    owner_slug: OwnerSlug::new("content").unwrap(),
                    resource_kind: ResourceKind::new("article").unwrap(),
                    resource_id: ResourceId::new("article-1").unwrap(),
                    subresource_id: None,
                },
                display_label: "Article".to_string(),
                lifecycle: TranslationResourceLifecycle::Active,
                resource_revision: OpaqueRevision::new("7").unwrap(),
                exact_locales: vec![TenantLocale::new("en").unwrap()],
            },
            source_locale: TenantLocale::new("en").unwrap(),
            target_locale: TenantLocale::new("de").unwrap(),
            rendered_fallback_locale: Some(TenantLocale::new("en").unwrap()),
            source_revision: OpaqueRevision::new("7:en").unwrap(),
            target_revision: None,
            fields: vec![TranslationFieldSnapshot {
                descriptor: TranslationFieldDescriptor {
                    key: field_key,
                    profile: TranslationValueProfile::PlainText,
                    strategy: TranslationStrategy::Translate,
                    classification: TranslationDataClassification::Public,
                    required: true,
                    ai_export_allowed: true,
                    max_characters: Some(200),
                    preserves_whitespace: false,
                },
                source_value: "Source".to_string(),
                exact_target_value: None,
                source_hash: "sha256:source".to_string(),
                protected_tokens: Vec::new(),
            }],
        };

        snapshot.validate().unwrap();
        assert!(
            !snapshot
                .summary
                .exact_locales
                .contains(&snapshot.target_locale)
        );
        assert_eq!(
            snapshot.rendered_fallback_locale.as_ref().unwrap().as_str(),
            "en"
        );
    }

    #[test]
    fn tenant_locale_type_rejects_unknown_provenance() {
        assert!(TenantLocale::new("und").is_err());
    }

    #[test]
    fn aggregate_progress_rejects_impossible_counts_and_equal_locales() {
        let request = TranslationTargetProgressRequest {
            source_locale: TenantLocale::new("en").unwrap(),
            target_locale: TenantLocale::new("en").unwrap(),
        };
        assert_eq!(
            request.validate(),
            Err(TranslationTargetContractError::EqualSourceAndTargetLocale)
        );

        let facts = TranslationTargetProgressFacts {
            required_units: 1,
            exact_required_units: 2,
            optional_units: 3,
            exact_optional_units: 3,
            resources: 1,
            complete_resources: 1,
            owner_change_cursor: None,
        };
        assert_eq!(
            facts.validate(),
            Err(TranslationTargetContractError::ExactRequiredUnitsOverflow)
        );
    }

    #[test]
    fn snapshot_requires_a_unique_source_backed_protected_token_ledger() {
        let mut snapshot = TranslationResourceSnapshot {
            summary: TranslationResourceSummary {
                identity: TranslationResourceIdentity {
                    owner_slug: OwnerSlug::new("content").unwrap(),
                    resource_kind: ResourceKind::new("template").unwrap(),
                    resource_id: ResourceId::new("welcome").unwrap(),
                    subresource_id: None,
                },
                display_label: "Welcome".to_string(),
                lifecycle: TranslationResourceLifecycle::Active,
                resource_revision: OpaqueRevision::new("1").unwrap(),
                exact_locales: vec![TenantLocale::new("en").unwrap()],
            },
            source_locale: TenantLocale::new("en").unwrap(),
            target_locale: TenantLocale::new("de").unwrap(),
            rendered_fallback_locale: None,
            source_revision: OpaqueRevision::new("1:en").unwrap(),
            target_revision: None,
            fields: vec![TranslationFieldSnapshot {
                descriptor: TranslationFieldDescriptor {
                    key: FieldKey::new("body").unwrap(),
                    profile: TranslationValueProfile::TemplateText,
                    strategy: TranslationStrategy::TranslateWithPlaceholders,
                    classification: TranslationDataClassification::TenantPrivate,
                    required: true,
                    ai_export_allowed: true,
                    max_characters: None,
                    preserves_whitespace: true,
                },
                source_value: "Hello {name}".to_string(),
                exact_target_value: None,
                source_hash: "sha256:template".to_string(),
                protected_tokens: vec!["{name}".to_string()],
            }],
        };
        snapshot.validate().unwrap();

        snapshot.fields[0].protected_tokens = vec!["{missing}".to_string()];
        assert_eq!(
            snapshot.validate(),
            Err(TranslationTargetContractError::ProtectedTokenMissingFromSource)
        );
        snapshot.fields[0].protected_tokens = vec!["{name}".to_string(), "{name}".to_string()];
        assert_eq!(
            snapshot.validate(),
            Err(TranslationTargetContractError::DuplicateProtectedToken)
        );
    }

    #[test]
    fn protected_token_helpers_preserve_unique_ledger_counts_and_whitespace_shape() {
        let expected = vec!["{name}".to_string(), "{count}".to_string()];
        assert!(protected_token_ledger_matches(
            &expected,
            &["{count}".to_string(), "{name}".to_string()]
        ));
        assert!(!protected_token_ledger_matches(
            &expected,
            &[
                "{name}".to_string(),
                "{name}".to_string(),
                "{count}".to_string(),
            ]
        ));
        assert!(protected_token_multiplicities_match(
            "Hello {name} {name}",
            "Hallo {name} {name}",
            &["{name}".to_string()]
        ));
        assert!(!protected_token_multiplicities_match(
            "Hello {name} {name}",
            "Hallo {name}",
            &["{name}".to_string()]
        ));
        assert!(whitespace_shape_matches("  Hello\r\n", "  Hallo\r\n"));
        assert!(!whitespace_shape_matches("  Hello\r\n", "Hallo\n"));
    }

    #[test]
    fn patch_validation_acceptance_matches_typed_issue_severity() {
        let invalid = TranslationPatchValidation {
            accepted: true,
            issues: vec![TranslationPatchIssue {
                field: None,
                severity: TranslationPatchIssueSeverity::Error,
                code: "owner.conflict".to_string(),
                message: "owner state changed".to_string(),
            }],
        };
        assert_eq!(
            invalid.validate(),
            Err(TranslationTargetContractError::PatchValidationAcceptanceMismatch)
        );

        TranslationPatchValidation {
            accepted: true,
            issues: vec![TranslationPatchIssue {
                field: None,
                severity: TranslationPatchIssueSeverity::Warning,
                code: "owner.warning".to_string(),
                message: "review this value".to_string(),
            }],
        }
        .validate()
        .unwrap();
    }

    #[test]
    fn list_contract_is_bounded_and_exact_locales_must_differ() {
        let request = ListTranslationResourcesRequest {
            source_locale: TenantLocale::new("en").unwrap(),
            target_locale: TenantLocale::new("de").unwrap(),
            cursor: None,
            limit: MAX_TRANSLATION_RESOURCE_PAGE_SIZE,
        };
        request.validate().unwrap();

        let mut invalid = request;
        invalid.limit = MAX_TRANSLATION_RESOURCE_PAGE_SIZE + 1;
        assert_eq!(
            invalid.validate(),
            Err(TranslationTargetContractError::InvalidPageLimit)
        );
    }

    #[test]
    fn apply_context_requires_write_deadline_and_idempotency() {
        let context = PortContext::new(
            "00000000-0000-0000-0000-000000000001",
            rustok_api::PortActor::service("translation"),
            "en",
            "translation-test",
        )
        .with_deadline(std::time::Duration::from_secs(1));
        let error = validate_translation_apply_context(&context).unwrap_err();
        assert_eq!(error.code, "port.idempotency_key_required");

        validate_translation_apply_context(&context.with_idempotency_key("translation-apply-1"))
            .unwrap();
    }
}
