use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use cron::Schedule;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    str::FromStr,
};
use thiserror::Error;

use rustok_api::{
    ArtifactPermissionLocalization, is_valid_locale_tag, is_valid_module_slug,
    manifest_hash::hash_manifest_snapshot,
};
use rustok_sandbox::{CapabilityName, RHAI_SOURCE_MEDIA_TYPE, SandboxExecutorKind};

use crate::{artifact_schema::validates_ui_contribution, build::ModuleBuildRequest};

/// The current immutable descriptor contract. Schema documents are bundled in
/// v4 so no admission or execution path needs to resolve schemas from a
/// network, filesystem, registry tag, or mutable service.
pub const MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION: u32 = 4;
/// Canonical author-source manifest consumed by the isolated build worker.
pub const MODULE_ARTIFACT_SOURCE_MANIFEST_FILE: &str = "module-artifact.json";
/// Source manifests use the same bounded envelope as finalized descriptors.
pub const MAX_MODULE_ARTIFACT_SOURCE_MANIFEST_BYTES: usize = 256 * 1024;

const MAX_SCHEMA_DOCUMENTS: usize = 32;
const MAX_SCHEMA_DOCUMENT_BYTES: usize = 64 * 1024;
const MAX_LOCALIZATION_CATALOGS: usize = 8;
const MAX_LOCALIZATION_LOCALES: usize = 8;
const MAX_LOCALIZATION_MESSAGES: usize = 256;
const MAX_LOCALIZATION_MESSAGE_BYTES: usize = 4 * 1024;
const MAX_UI_CONTRIBUTIONS: usize = 32;
const JSON_SCHEMA_DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

/// Stable OCI layer media type for immutable Rhai source artifacts.
pub const MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE: &str = RHAI_SOURCE_MEDIA_TYPE;
/// Stable OCI layer media type for immutable WebAssembly Component artifacts.
pub const MODULE_ARTIFACT_WASM_COMPONENT_MEDIA_TYPE: &str =
    "application/vnd.rustok.wasm.component.v1+wasm";
/// Stable OCI layer media type for immutable sidecar metadata artifacts.
pub const MODULE_ARTIFACT_SIDECAR_MEDIA_TYPE: &str = "application/vnd.rustok.sidecar.v1";
/// Stable OCI layer media type for immutable static-promotion source references.
pub const MODULE_ARTIFACT_STATIC_PROMOTION_MEDIA_TYPE: &str =
    "application/vnd.rustok.static-promotion.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactPayloadKind {
    Rhai,
    WasmComponent,
    StaticPromoted,
    Sidecar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactModuleKind {
    Core,
    Optional,
}

impl ArtifactPayloadKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rhai => "rhai",
            Self::WasmComponent => "wasm_component",
            Self::StaticPromoted => "static_promoted",
            Self::Sidecar => "sidecar",
        }
    }

    /// Default OCI layer media type for this immutable payload kind. Rhai also
    /// permits the workspace representation when admission records it exactly.
    pub const fn oci_layer_media_type(self) -> &'static str {
        match self {
            Self::Rhai => MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE,
            Self::WasmComponent => MODULE_ARTIFACT_WASM_COMPONENT_MEDIA_TYPE,
            Self::StaticPromoted => MODULE_ARTIFACT_STATIC_PROMOTION_MEDIA_TYPE,
            Self::Sidecar => MODULE_ARTIFACT_SIDECAR_MEDIA_TYPE,
        }
    }

    /// Whether an admitted OCI layer media type is valid for this payload kind.
    /// Rhai accepts either one source document or its canonical workspace form.
    pub fn supports_media_type(self, media_type: &str) -> bool {
        match self {
            Self::Rhai => matches!(
                media_type,
                MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE | rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE
            ),
            _ => media_type == self.oci_layer_media_type(),
        }
    }

    /// Static promotion is intentionally outside the isolated executor process.
    pub fn sandbox_executor(self) -> Option<SandboxExecutorKind> {
        match self {
            Self::Rhai => Some(SandboxExecutorKind::Rhai),
            Self::WasmComponent => Some(SandboxExecutorKind::WasmComponent),
            Self::Sidecar => Some(SandboxExecutorKind::Sidecar),
            Self::StaticPromoted => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactOrigin {
    AlloyDraft,
    Marketplace,
    FirstParty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReleaseRef {
    pub slug: String,
    pub version: String,
    pub digest: String,
}

impl ArtifactReleaseRef {
    pub fn validate(&self) -> Result<(), ModuleArtifactError> {
        if !is_valid_module_slug(&self.slug) {
            return Err(ModuleArtifactError::InvalidSlug(self.slug.clone()));
        }
        Version::parse(&self.version)
            .map_err(|_| ModuleArtifactError::InvalidVersion(self.version.clone()))?;
        if !valid_digest(&self.digest) {
            return Err(ModuleArtifactError::InvalidDigest(self.digest.clone()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSourceLineage {
    pub origin: ArtifactOrigin,
    pub source_digest: String,
    pub parent_release: Option<ArtifactReleaseRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleArtifactDescriptor {
    pub schema_version: u32,
    pub slug: String,
    pub version: String,
    pub payload_kind: ArtifactPayloadKind,
    pub module_kind: ArtifactModuleKind,
    pub runtime_abi: String,
    pub platform_compatibility: String,
    #[serde(default)]
    pub required_features: Vec<String>,
    /// Build-derived component digest. It is absent from an author-source
    /// manifest and mandatory on every finalized descriptor.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub artifact_digest: String,
    pub entrypoint: String,
    #[serde(default)]
    pub capabilities: Vec<CapabilityName>,
    #[serde(default)]
    pub bindings: Vec<ModuleRuntimeBinding>,
    #[serde(default)]
    pub dependencies: Vec<ModuleDependencyConstraint>,
    #[serde(default)]
    pub permissions: Vec<ArtifactPermissionDescriptor>,
    #[serde(default)]
    pub schema_documents: Vec<ArtifactSchemaDocument>,
    #[serde(default)]
    pub settings_schema_digest: Option<String>,
    #[serde(default)]
    pub data_schema_digest: Option<String>,
    #[serde(default)]
    pub localization_catalogs: Vec<ArtifactLocalizationCatalog>,
    #[serde(default)]
    pub ui_contributions: Vec<ArtifactUiContribution>,
    #[serde(default)]
    pub persistence_contract: Option<ArtifactPersistenceContract>,
}

/// Validated type-state for the author-controlled descriptor declaration.
/// It cannot carry the component digest that is known only after the worker
/// has inspected the fixed build output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleArtifactSourceManifest {
    descriptor: ModuleArtifactDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModuleArtifactSourceManifestError {
    #[error("module artifact source manifest is invalid")]
    Invalid,
    #[error("module artifact source manifest must not declare build-derived artifact digest")]
    BuildDerivedDigest,
    #[error("module artifact source manifest descriptor is invalid")]
    Descriptor(#[source] ModuleArtifactError),
    #[error("module artifact source manifest does not match the immutable build request")]
    BuildIdentityMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleDependencyConstraint {
    pub slug: String,
    pub version_requirement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactPermissionDescriptor {
    pub key: String,
    /// Immutable localized operator metadata registered by the RBAC owner.
    pub localizations: Vec<ArtifactPermissionLocalization>,
}

/// One immutable Draft 2020-12 JSON Schema document bundled with the artifact
/// descriptor. The digest is over canonical JSON, not transport bytes, so it
/// is stable across object-key ordering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSchemaDocument {
    pub digest: String,
    pub document: Value,
}

/// Immutable plain-text localization catalog admitted with an artifact
/// descriptor. Hosts select one effective locale; this contract deliberately
/// has no artifact-controlled fallback chain, markup, or rendering behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactLocalizationCatalog {
    pub digest: String,
    pub messages: BTreeMap<String, BTreeMap<String, String>>,
}

impl ArtifactLocalizationCatalog {
    /// Resolves only the exact host-selected locale. Hosts must not substitute
    /// a module-provided fallback locale when this lookup misses.
    pub fn message(&self, locale: &str, key: &str) -> Option<&str> {
        self.messages
            .get(locale)
            .and_then(|messages| messages.get(key))
            .map(String::as_str)
    }

    fn validate(&self) -> bool {
        if !valid_digest(&self.digest)
            || self.digest
                != canonical_schema_digest(
                    &serde_json::to_value(&self.messages)
                        .expect("localization catalog serialization must be infallible"),
                )
            || self.messages.is_empty()
            || self.messages.len() > MAX_LOCALIZATION_LOCALES
        {
            return false;
        }

        let mut expected_keys = None::<BTreeSet<&str>>;
        for (locale, messages) in &self.messages {
            if !is_valid_locale_tag(locale)
                || messages.is_empty()
                || messages.len() > MAX_LOCALIZATION_MESSAGES
                || messages.iter().any(|(key, value)| {
                    !valid_localization_key(key) || !valid_localization_message(value)
                })
            {
                return false;
            }
            let keys = messages.keys().map(String::as_str).collect::<BTreeSet<_>>();
            if expected_keys
                .as_ref()
                .is_some_and(|expected| expected != &keys)
            {
                return false;
            }
            expected_keys = Some(keys);
        }
        true
    }

    fn contains_key_for_every_locale(&self, key: &str) -> bool {
        self.messages
            .values()
            .all(|messages| messages.contains_key(key))
    }
}

/// Host surface on which one declarative contribution may render. The values
/// name a platform presentation slot, never a host package, URL, iframe, or
/// executable component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactUiSurface {
    AdminSettings,
    AdminActions,
    AdminStatus,
    AdminHelp,
    AdminNavigation,
    AdminTable,
    AdminForm,
    StorefrontSlot,
}

/// User confirmation required before an action/form dispatches its admitted
/// runtime binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactUiActionConfirmation {
    None,
    Acknowledge,
    Destructive,
}

/// Every declarative action is audit-required. This explicit protocol field
/// prevents an artifact from opting a host action out of durable evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactUiAuditPolicy {
    Required,
}

/// Framework-neutral content that a host may render from shared primitives.
/// It contains no component source, HTML, CSS, URL, query, locale fallback,
/// authentication behavior, or direct host API request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArtifactUiContributionContent {
    Settings {
        title_key: String,
        settings_schema_digest: String,
    },
    Action {
        title_key: String,
        binding_id: String,
        confirmation: ArtifactUiActionConfirmation,
        destructive: bool,
        audit_policy: ArtifactUiAuditPolicy,
    },
    Status {
        title_key: String,
        status_key: String,
    },
    Help {
        title_key: String,
        body_key: String,
    },
    Navigation {
        title_key: String,
        route: String,
    },
    Table {
        title_key: String,
        schema_digest: String,
    },
    Form {
        title_key: String,
        binding_id: String,
        confirmation: ArtifactUiActionConfirmation,
        destructive: bool,
        audit_policy: ArtifactUiAuditPolicy,
    },
    StorefrontSlot {
        title_key: String,
        slot: String,
    },
}

impl ArtifactUiContributionContent {
    fn expected_surface(&self) -> ArtifactUiSurface {
        match self {
            Self::Settings { .. } => ArtifactUiSurface::AdminSettings,
            Self::Action { .. } => ArtifactUiSurface::AdminActions,
            Self::Status { .. } => ArtifactUiSurface::AdminStatus,
            Self::Help { .. } => ArtifactUiSurface::AdminHelp,
            Self::Navigation { .. } => ArtifactUiSurface::AdminNavigation,
            Self::Table { .. } => ArtifactUiSurface::AdminTable,
            Self::Form { .. } => ArtifactUiSurface::AdminForm,
            Self::StorefrontSlot { .. } => ArtifactUiSurface::StorefrontSlot,
        }
    }

    fn localization_keys(&self) -> Vec<&str> {
        match self {
            Self::Settings { title_key, .. }
            | Self::Action { title_key, .. }
            | Self::Status { title_key, .. }
            | Self::Help { title_key, .. }
            | Self::Navigation { title_key, .. }
            | Self::Table { title_key, .. }
            | Self::Form { title_key, .. }
            | Self::StorefrontSlot { title_key, .. } => {
                let mut keys = vec![title_key.as_str()];
                match self {
                    Self::Status { status_key, .. } => keys.push(status_key),
                    Self::Help { body_key, .. } => keys.push(body_key),
                    _ => {}
                }
                keys
            }
        }
    }
}

/// Host-rendered declarative contribution. It is metadata only: marketplace
/// artifacts never inject executable UI code into a host process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactUiContribution {
    pub id: String,
    pub surface: ArtifactUiSurface,
    pub localization_digest: String,
    pub permission: String,
    pub content: ArtifactUiContributionContent,
}

/// Metadata for brokered namespaced data only. It never carries SQL, DDL, or
/// executable migrations from an untrusted marketplace artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactPersistenceContract {
    pub revision: u64,
    pub schema_digest: String,
    /// Bounded logical indexes that the platform may materialize in its
    /// private namespace. They never carry physical table, column, or SQL
    /// identity from an artifact.
    #[serde(default)]
    pub indexes: Vec<ArtifactDataIndexField>,
}

/// One host-materialized scalar projection from brokered JSON data. The JSON
/// pointer grammar is intentionally narrow so it cannot become a database JSON
/// path, SQL fragment, or query expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDataIndexField {
    pub name: String,
    pub json_pointer: String,
    pub value_type: ArtifactDataIndexValueType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactDataIndexValueType {
    String,
    Number,
    Boolean,
}

/// Declarative runtime binding admitted with an immutable artifact descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleRuntimeBinding {
    pub id: String,
    pub kind: ModuleRuntimeBindingKind,
    pub entrypoint: String,
    pub input_schema_digest: String,
    pub output_schema_digest: String,
    /// Exact module-owned RBAC permission required to invoke this binding.
    /// Capability grants constrain guest-to-host calls separately and never
    /// authorize an actor to invoke a binding.
    pub permission: String,
    pub idempotency: ModuleBindingIdempotency,
    pub limit_profile: String,
    #[serde(default)]
    pub capabilities: Vec<CapabilityName>,
    /// Exact or terminal-wildcard event topics. This is populated only for an
    /// Event binding; it is not a generic routing expression.
    #[serde(default)]
    pub event_topics: Vec<String>,
    /// Platform scheduler contract. This is populated only for a Schedule
    /// binding and never contains a host-selected queue or timer identifier.
    #[serde(default)]
    pub schedule: Option<ModuleScheduleBinding>,
    /// Platform-owned HTTP contract. This is populated only for an HTTP
    /// binding; artifacts cannot provide routers, listener ports, headers, or
    /// streaming transports.
    #[serde(default)]
    pub http: Option<ModuleHttpBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleRuntimeBindingKind {
    PreEnable,
    PostEnable,
    PreDisable,
    PostDisable,
    Command,
    Http,
    Event,
    Schedule,
    Health,
    Readiness,
    ActivationSmoke,
    /// Owner-invoked transformation for a bounded data-contract upgrade page.
    /// It is not a generic user command or a lifecycle hook.
    DataUpgrade,
    BeforeCommit,
    AfterCommit,
    OnCommit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleScheduleBinding {
    pub cron: String,
    pub timezone: String,
    pub misfire: ModuleScheduleMisfirePolicy,
    pub overlap: ModuleScheduleOverlapPolicy,
    pub deduplication: ModuleScheduleDeduplication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleScheduleMisfirePolicy {
    Skip,
    RunOnce,
    CatchUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleScheduleOverlapPolicy {
    Forbid,
    Queue,
    Allow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleScheduleDeduplication {
    None,
    PerSlot,
}

/// A host-owned HTTP route. `path` is relative to the platform's module route
/// namespace and consists solely of literal path segments in v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleHttpBinding {
    pub method: ModuleHttpMethod,
    pub path: String,
    pub request_media_type: String,
    pub response_media_type: String,
    pub max_body_bytes: u64,
    pub max_output_bytes: u64,
    pub timeout_ms: u64,
    pub streaming: ModuleHttpStreamingPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleHttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleHttpStreamingPolicy {
    Forbidden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleBindingIdempotency {
    Required,
    BestEffort,
    None,
}

impl ModuleArtifactDescriptor {
    pub fn validate(&self) -> Result<(), ModuleArtifactError> {
        if !valid_digest(&self.artifact_digest) {
            return Err(ModuleArtifactError::InvalidDigest(
                self.artifact_digest.clone(),
            ));
        }
        self.validate_declaration()
    }

    fn validate_declaration(&self) -> Result<(), ModuleArtifactError> {
        if !is_valid_module_slug(&self.slug) {
            return Err(ModuleArtifactError::InvalidSlug(self.slug.clone()));
        }
        Version::parse(&self.version)
            .map_err(|error| ModuleArtifactError::InvalidVersion(error.to_string()))?;
        if self.schema_version != MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION {
            return Err(ModuleArtifactError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        VersionReq::parse(&self.platform_compatibility).map_err(|_| {
            ModuleArtifactError::InvalidPlatformCompatibility(self.platform_compatibility.clone())
        })?;
        for feature in &self.required_features {
            if feature.trim().is_empty() || feature.contains(char::is_whitespace) {
                return Err(ModuleArtifactError::InvalidRequiredFeature(feature.clone()));
            }
        }
        if self.runtime_abi.trim().is_empty() {
            return Err(ModuleArtifactError::MissingRuntimeAbi);
        }
        if self.entrypoint.trim().is_empty() {
            return Err(ModuleArtifactError::MissingEntrypoint);
        }
        for (index, capability) in self.capabilities.iter().enumerate() {
            if self.capabilities[..index]
                .iter()
                .any(|previous| previous == capability)
            {
                return Err(ModuleArtifactError::DuplicateCapability(
                    capability.as_str().to_string(),
                ));
            }
        }
        let schema_digests = self.validate_schema_bundle()?;
        for (index, binding) in self.bindings.iter().enumerate() {
            if binding.id.trim().is_empty()
                || binding.entrypoint.trim().is_empty()
                || !self
                    .permissions
                    .iter()
                    .any(|permission| permission.key == binding.permission)
            {
                return Err(ModuleArtifactError::InvalidBinding(binding.id.clone()));
            }
            if !valid_digest(&binding.input_schema_digest)
                || !valid_digest(&binding.output_schema_digest)
            {
                return Err(ModuleArtifactError::InvalidBindingSchemaDigest(
                    binding.id.clone(),
                ));
            }
            if self.bindings[..index]
                .iter()
                .any(|previous| previous.id == binding.id)
            {
                return Err(ModuleArtifactError::DuplicateBinding(binding.id.clone()));
            }
            if binding
                .capabilities
                .iter()
                .any(|capability| !self.capabilities.contains(capability))
            {
                return Err(ModuleArtifactError::UndeclaredBindingCapability(
                    binding.id.clone(),
                ));
            }
            if !schema_digests.contains(&binding.input_schema_digest)
                || !schema_digests.contains(&binding.output_schema_digest)
            {
                return Err(ModuleArtifactError::MissingSchemaDocument(
                    binding.id.clone(),
                ));
            }
            if (binding.kind == ModuleRuntimeBindingKind::Event
                && (binding.event_topics.is_empty()
                    || binding.event_topics.len() > 32
                    || binding
                        .event_topics
                        .iter()
                        .any(|topic| !valid_event_topic(topic))))
                || (binding.kind != ModuleRuntimeBindingKind::Event
                    && !binding.event_topics.is_empty())
            {
                return Err(ModuleArtifactError::InvalidBinding(binding.id.clone()));
            }
            if binding
                .event_topics
                .iter()
                .enumerate()
                .any(|(topic_index, topic)| {
                    binding.event_topics[..topic_index]
                        .iter()
                        .any(|previous| previous == topic)
                })
            {
                return Err(ModuleArtifactError::InvalidBinding(binding.id.clone()));
            }
            match (binding.kind.clone(), &binding.schedule) {
                (ModuleRuntimeBindingKind::Schedule, Some(schedule)) if schedule.validate() => {}
                (ModuleRuntimeBindingKind::Schedule, _) | (_, Some(_)) => {
                    return Err(ModuleArtifactError::InvalidBinding(binding.id.clone()));
                }
                (_, None) => {}
            }
            match (binding.kind.clone(), &binding.http) {
                (ModuleRuntimeBindingKind::Http, Some(http)) if http.validate() => {}
                (ModuleRuntimeBindingKind::Http, _) | (_, Some(_)) => {
                    return Err(ModuleArtifactError::InvalidBinding(binding.id.clone()));
                }
                (_, None) => {}
            }
            if let Some(http) = &binding.http
                && self.bindings[..index].iter().any(|previous| {
                    previous.http.as_ref().is_some_and(|previous_http| {
                        previous_http.method == http.method && previous_http.path == http.path
                    })
                })
            {
                return Err(ModuleArtifactError::InvalidBinding(binding.id.clone()));
            }
        }
        for (index, dependency) in self.dependencies.iter().enumerate() {
            if !is_valid_module_slug(&dependency.slug) || dependency.slug == self.slug {
                return Err(ModuleArtifactError::InvalidDependency(
                    dependency.slug.clone(),
                ));
            }
            VersionReq::parse(&dependency.version_requirement).map_err(|_| {
                ModuleArtifactError::InvalidDependencyVersionRequirement {
                    slug: dependency.slug.clone(),
                    requirement: dependency.version_requirement.clone(),
                }
            })?;
            if self.dependencies[..index]
                .iter()
                .any(|previous| previous.slug == dependency.slug)
            {
                return Err(ModuleArtifactError::DuplicateDependency(
                    dependency.slug.clone(),
                ));
            }
        }
        let permission_prefix = format!("{}.", self.slug);
        for (index, permission) in self.permissions.iter().enumerate() {
            if !permission.key.starts_with(&permission_prefix)
                || permission.localizations.is_empty()
                || permission.localizations.iter().any(|localization| {
                    !is_valid_locale_tag(&localization.locale)
                        || localization.label.trim().is_empty()
                        || localization.description.trim().is_empty()
                })
                || permission.localizations.iter().enumerate().any(
                    |(localization_index, localization)| {
                        permission.localizations[..localization_index]
                            .iter()
                            .any(|previous| previous.locale == localization.locale)
                    },
                )
            {
                return Err(ModuleArtifactError::InvalidPermission(
                    permission.key.clone(),
                ));
            }
            if self.permissions[..index]
                .iter()
                .any(|previous| previous.key == permission.key)
            {
                return Err(ModuleArtifactError::DuplicatePermission(
                    permission.key.clone(),
                ));
            }
        }
        for selector in [&self.settings_schema_digest, &self.data_schema_digest]
            .into_iter()
            .flatten()
        {
            if !valid_digest(selector) || !schema_digests.contains(selector) {
                return Err(ModuleArtifactError::MissingSchemaDocument(selector.clone()));
            }
        }
        if self.localization_catalogs.len() > MAX_LOCALIZATION_CATALOGS {
            return Err(ModuleArtifactError::TooManyLocalizationCatalogs);
        }
        for (index, catalog) in self.localization_catalogs.iter().enumerate() {
            if !catalog.validate() {
                return Err(ModuleArtifactError::InvalidLocalizationCatalog(
                    catalog.digest.clone(),
                ));
            }
            if self.localization_catalogs[..index]
                .iter()
                .any(|previous| previous.digest == catalog.digest)
            {
                return Err(ModuleArtifactError::DuplicateLocalizationCatalog(
                    catalog.digest.clone(),
                ));
            }
        }
        if self.ui_contributions.len() > MAX_UI_CONTRIBUTIONS {
            return Err(ModuleArtifactError::TooManyUiContributions);
        }
        let mut navigation_routes = HashSet::new();
        let mut storefront_slots = HashSet::new();
        for (index, contribution) in self.ui_contributions.iter().enumerate() {
            if !is_valid_module_slug(&contribution.id)
                || !valid_digest(&contribution.localization_digest)
                || contribution.surface != contribution.content.expected_surface()
                || !contribution.permission.starts_with(&permission_prefix)
                || !self
                    .permissions
                    .iter()
                    .any(|permission| permission.key == contribution.permission)
                || !serde_json::to_value(contribution)
                    .ok()
                    .is_some_and(|value| validates_ui_contribution(&value))
            {
                return Err(ModuleArtifactError::InvalidUiContribution(
                    contribution.id.clone(),
                ));
            }
            if self.ui_contributions[..index]
                .iter()
                .any(|previous| previous.id == contribution.id)
            {
                return Err(ModuleArtifactError::DuplicateUiContribution(
                    contribution.id.clone(),
                ));
            }
            let catalog = self
                .localization_catalogs
                .iter()
                .find(|catalog| catalog.digest == contribution.localization_digest)
                .ok_or_else(|| {
                    ModuleArtifactError::MissingLocalizationCatalog(
                        contribution.localization_digest.clone(),
                    )
                })?;
            if contribution
                .content
                .localization_keys()
                .iter()
                .any(|key| !catalog.contains_key_for_every_locale(key))
            {
                return Err(ModuleArtifactError::InvalidUiContribution(
                    contribution.id.clone(),
                ));
            }
            match &contribution.content {
                ArtifactUiContributionContent::Settings {
                    settings_schema_digest,
                    ..
                } => {
                    if self.settings_schema_digest.as_deref() != Some(settings_schema_digest)
                        || !schema_digests.contains(settings_schema_digest)
                    {
                        return Err(ModuleArtifactError::InvalidUiContribution(
                            contribution.id.clone(),
                        ));
                    }
                }
                ArtifactUiContributionContent::Action {
                    binding_id,
                    confirmation,
                    destructive,
                    audit_policy,
                    ..
                }
                | ArtifactUiContributionContent::Form {
                    binding_id,
                    confirmation,
                    destructive,
                    audit_policy,
                    ..
                } => {
                    let binding = self
                        .bindings
                        .iter()
                        .find(|binding| binding.id == *binding_id);
                    if binding.is_none_or(|binding| {
                        binding.kind != ModuleRuntimeBindingKind::Command
                            || binding.permission != contribution.permission
                            || binding.idempotency != ModuleBindingIdempotency::Required
                            || *audit_policy != ArtifactUiAuditPolicy::Required
                            || (*destructive
                                != (*confirmation == ArtifactUiActionConfirmation::Destructive))
                    }) {
                        return Err(ModuleArtifactError::InvalidUiContribution(
                            contribution.id.clone(),
                        ));
                    }
                }
                ArtifactUiContributionContent::Table { schema_digest, .. } => {
                    if !valid_digest(schema_digest) || !schema_digests.contains(schema_digest) {
                        return Err(ModuleArtifactError::InvalidUiContribution(
                            contribution.id.clone(),
                        ));
                    }
                }
                ArtifactUiContributionContent::Navigation { route, .. } => {
                    if !valid_ui_route(route) || !navigation_routes.insert(route.as_str()) {
                        return Err(ModuleArtifactError::InvalidUiContribution(
                            contribution.id.clone(),
                        ));
                    }
                }
                ArtifactUiContributionContent::StorefrontSlot { slot, .. } => {
                    if !is_valid_module_slug(slot) || !storefront_slots.insert(slot.as_str()) {
                        return Err(ModuleArtifactError::InvalidUiContribution(
                            contribution.id.clone(),
                        ));
                    }
                }
                ArtifactUiContributionContent::Status { .. }
                | ArtifactUiContributionContent::Help { .. } => {}
            }
        }
        if let Some(contract) = &self.persistence_contract {
            if contract.revision == 0
                || !valid_digest(&contract.schema_digest)
                || !schema_digests.contains(&contract.schema_digest)
                || contract.indexes.len() > 16
            {
                return Err(ModuleArtifactError::InvalidPersistenceContract);
            }
            let mut index_names = HashSet::with_capacity(contract.indexes.len());
            for index in &contract.indexes {
                if !valid_data_index_name(&index.name)
                    || !valid_data_index_pointer(&index.json_pointer)
                    || !index_names.insert(&index.name)
                {
                    return Err(ModuleArtifactError::InvalidPersistenceContract);
                }
            }
        }
        Ok(())
    }

    pub fn release_ref(&self) -> ArtifactReleaseRef {
        ArtifactReleaseRef {
            slug: self.slug.clone(),
            version: self.version.clone(),
            digest: self.artifact_digest.clone(),
        }
    }

    /// Looks up a schema document by its immutable canonical digest.
    pub fn schema_document(&self, digest: &str) -> Option<&Value> {
        self.schema_documents
            .iter()
            .find(|schema| schema.digest == digest)
            .map(|schema| &schema.document)
    }

    pub fn settings_schema(&self) -> Option<&Value> {
        self.settings_schema_digest
            .as_deref()
            .and_then(|digest| self.schema_document(digest))
    }

    pub fn data_schema(&self) -> Option<&Value> {
        self.data_schema_digest
            .as_deref()
            .and_then(|digest| self.schema_document(digest))
    }

    fn validate_schema_bundle(
        &self,
    ) -> Result<std::collections::BTreeSet<String>, ModuleArtifactError> {
        if self.schema_documents.len() > MAX_SCHEMA_DOCUMENTS {
            return Err(ModuleArtifactError::TooManySchemaDocuments);
        }
        let mut digests = std::collections::BTreeSet::new();
        for schema in &self.schema_documents {
            if !valid_digest(&schema.digest) || !schema.document.is_object() {
                return Err(ModuleArtifactError::InvalidSchemaDocument(
                    schema.digest.clone(),
                ));
            }
            let encoded = serde_json::to_vec(&schema.document)
                .map_err(|_| ModuleArtifactError::InvalidSchemaDocument(schema.digest.clone()))?;
            if encoded.len() > MAX_SCHEMA_DOCUMENT_BYTES {
                return Err(ModuleArtifactError::SchemaDocumentTooLarge(
                    schema.digest.clone(),
                ));
            }
            if schema
                .document
                .as_object()
                .and_then(|document| document.get("$schema"))
                .and_then(Value::as_str)
                != Some(JSON_SCHEMA_DRAFT_2020_12)
            {
                return Err(ModuleArtifactError::InvalidSchemaDocument(
                    schema.digest.clone(),
                ));
            }
            validate_local_schema_references(&schema.document)?;
            let actual = canonical_schema_digest(&schema.document);
            if schema.digest != actual {
                return Err(ModuleArtifactError::SchemaDigestMismatch {
                    declared: schema.digest.clone(),
                    actual,
                });
            }
            if !digests.insert(schema.digest.clone()) {
                return Err(ModuleArtifactError::DuplicateSchemaDocument(
                    schema.digest.clone(),
                ));
            }
        }
        Ok(digests)
    }
}

impl ModuleArtifactSourceManifest {
    pub fn parse(bytes: &[u8]) -> Result<Self, ModuleArtifactSourceManifestError> {
        if bytes.is_empty() || bytes.len() > MAX_MODULE_ARTIFACT_SOURCE_MANIFEST_BYTES {
            return Err(ModuleArtifactSourceManifestError::Invalid);
        }
        let descriptor: ModuleArtifactDescriptor = serde_json::from_slice(bytes)
            .map_err(|_| ModuleArtifactSourceManifestError::Invalid)?;
        if !descriptor.artifact_digest.is_empty() {
            return Err(ModuleArtifactSourceManifestError::BuildDerivedDigest);
        }
        descriptor
            .validate_declaration()
            .map_err(ModuleArtifactSourceManifestError::Descriptor)?;
        Ok(Self { descriptor })
    }

    pub fn validate_build_request(
        &self,
        request: &ModuleBuildRequest,
    ) -> Result<(), ModuleArtifactSourceManifestError> {
        if self.descriptor.slug != request.expected_module_slug
            || self.descriptor.version != request.expected_version
            || self.descriptor.payload_kind != ArtifactPayloadKind::WasmComponent
            || self.descriptor.module_kind != ArtifactModuleKind::Optional
            || self.descriptor.runtime_abi != request.runtime_abi
            || self.descriptor.entrypoint != "run"
        {
            return Err(ModuleArtifactSourceManifestError::BuildIdentityMismatch);
        }
        Ok(())
    }

    pub fn slug(&self) -> &str {
        &self.descriptor.slug
    }

    pub fn version(&self) -> &str {
        &self.descriptor.version
    }

    pub fn runtime_abi(&self) -> &str {
        &self.descriptor.runtime_abi
    }

    pub fn entrypoint(&self) -> &str {
        &self.descriptor.entrypoint
    }

    pub fn capabilities(&self) -> &[CapabilityName] {
        &self.descriptor.capabilities
    }

    pub fn finalize(
        mut self,
        artifact_digest: String,
    ) -> Result<ModuleArtifactDescriptor, ModuleArtifactSourceManifestError> {
        self.descriptor.artifact_digest = artifact_digest;
        self.descriptor
            .validate()
            .map_err(ModuleArtifactSourceManifestError::Descriptor)?;
        Ok(self.descriptor)
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, ModuleArtifactSourceManifestError> {
        serde_json::to_vec_pretty(&self.descriptor)
            .map_err(|_| ModuleArtifactSourceManifestError::Invalid)
    }
}

/// Canonical digest of the complete immutable artifact descriptor.
///
/// This identity is distinct from the executable payload digest stored inside
/// the descriptor and from the OCI manifest digest that addresses the package.
pub fn canonical_artifact_descriptor_digest(descriptor: &ModuleArtifactDescriptor) -> String {
    let value = serde_json::to_value(descriptor)
        .expect("module artifact descriptor serialization must be infallible");
    format!("sha256:{}", hash_manifest_snapshot(&value))
}

fn valid_data_index_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_data_index_pointer(value: &str) -> bool {
    value.len() <= 128
        && value.starts_with('/')
        && value.split('/').skip(1).all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRelease {
    pub descriptor: ModuleArtifactDescriptor,
    pub lineage: ArtifactSourceLineage,
    pub published_at: DateTime<Utc>,
}

impl ArtifactRelease {
    /// Creates the next immutable release in this artifact's source lineage.
    ///
    /// A revision cannot silently change ownership to another module slug or
    /// overwrite the published version it was derived from.
    pub fn fork(
        &self,
        descriptor: ModuleArtifactDescriptor,
        source_digest: String,
    ) -> Result<ArtifactReleaseDraft, ModuleArtifactError> {
        descriptor.validate()?;
        if descriptor.slug != self.descriptor.slug {
            return Err(ModuleArtifactError::ForkSlugMismatch {
                expected: self.descriptor.slug.clone(),
                received: descriptor.slug,
            });
        }

        let parent_version = Version::parse(&self.descriptor.version)
            .expect("published artifact version must have been validated");
        let next_version =
            Version::parse(&descriptor.version).expect("validated artifact version must parse");
        if next_version <= parent_version {
            return Err(ModuleArtifactError::ForkVersionNotIncremented {
                parent: self.descriptor.version.clone(),
                received: descriptor.version,
            });
        }
        if descriptor.artifact_digest == self.descriptor.artifact_digest
            || source_digest == self.lineage.source_digest
        {
            return Err(ModuleArtifactError::ForkDigestNotChanged);
        }

        Ok(ArtifactReleaseDraft {
            descriptor,
            lineage: ArtifactSourceLineage {
                origin: ArtifactOrigin::AlloyDraft,
                source_digest,
                parent_release: Some(self.descriptor.release_ref()),
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactReleaseDraft {
    pub descriptor: ModuleArtifactDescriptor,
    pub lineage: ArtifactSourceLineage,
}

impl ArtifactReleaseDraft {
    pub fn publish(
        self,
        published_at: DateTime<Utc>,
    ) -> Result<ArtifactRelease, ModuleArtifactError> {
        self.descriptor.validate()?;
        if !valid_digest(&self.lineage.source_digest) {
            return Err(ModuleArtifactError::InvalidSourceDigest(
                self.lineage.source_digest,
            ));
        }
        Ok(ArtifactRelease {
            descriptor: self.descriptor,
            lineage: self.lineage,
            published_at,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModuleArtifactError {
    #[error("artifact slug `{0}` must be a short snake_case identifier")]
    InvalidSlug(String),
    #[error("artifact version is not valid semantic versioning: {0}")]
    InvalidVersion(String),
    #[error("artifact descriptor schema version `{0}` is unsupported")]
    UnsupportedSchemaVersion(u32),
    #[error("artifact platform compatibility requirement `{0}` is invalid")]
    InvalidPlatformCompatibility(String),
    #[error("artifact required platform feature `{0}` is invalid")]
    InvalidRequiredFeature(String),
    #[error("artifact runtime ABI must be declared")]
    MissingRuntimeAbi,
    #[error("artifact digest `{0}` must be a sha256 digest")]
    InvalidDigest(String),
    #[error("artifact source digest `{0}` must be a sha256 digest")]
    InvalidSourceDigest(String),
    #[error("artifact entrypoint must be declared")]
    MissingEntrypoint,
    #[error("artifact capability `{0}` is declared more than once")]
    DuplicateCapability(String),
    #[error("artifact binding `{0}` is invalid")]
    InvalidBinding(String),
    #[error("artifact binding `{0}` must declare sha256 input/output schemas")]
    InvalidBindingSchemaDigest(String),
    #[error(
        "artifact binding or selector `{0}` references a schema document absent from the descriptor"
    )]
    MissingSchemaDocument(String),
    #[error("artifact binding `{0}` is declared more than once")]
    DuplicateBinding(String),
    #[error("artifact binding `{0}` declares a capability absent from the descriptor")]
    UndeclaredBindingCapability(String),
    #[error("artifact dependency `{0}` is invalid")]
    InvalidDependency(String),
    #[error("artifact dependency `{slug}` has invalid semantic-version requirement `{requirement}")]
    InvalidDependencyVersionRequirement { slug: String, requirement: String },
    #[error("artifact dependency `{0}` is declared more than once")]
    DuplicateDependency(String),
    #[error("artifact permission `{0}` must use the owning module namespace and a label")]
    InvalidPermission(String),
    #[error("artifact permission `{0}` is declared more than once")]
    DuplicatePermission(String),
    #[error("artifact schema contains a non-local `$ref` `{0}")]
    NonLocalSchemaReference(String),
    #[error("artifact schema document `{0}` is invalid")]
    InvalidSchemaDocument(String),
    #[error("artifact schema document `{0}` exceeds the descriptor size limit")]
    SchemaDocumentTooLarge(String),
    #[error("artifact schema document `{0}` is declared more than once")]
    DuplicateSchemaDocument(String),
    #[error("artifact descriptor exceeds the schema-document limit")]
    TooManySchemaDocuments,
    #[error("artifact schema digest mismatch: declared `{declared}`, actual `{actual}")]
    SchemaDigestMismatch { declared: String, actual: String },
    #[error("artifact UI contribution `{0}` is invalid")]
    InvalidUiContribution(String),
    #[error("artifact UI contribution `{0}` is declared more than once")]
    DuplicateUiContribution(String),
    #[error("artifact descriptor exceeds the localization-catalog limit")]
    TooManyLocalizationCatalogs,
    #[error("artifact localization catalog `{0}` is invalid")]
    InvalidLocalizationCatalog(String),
    #[error("artifact localization catalog `{0}` is declared more than once")]
    DuplicateLocalizationCatalog(String),
    #[error("artifact UI contribution references unknown localization catalog `{0}`")]
    MissingLocalizationCatalog(String),
    #[error("artifact descriptor exceeds the UI-contribution limit")]
    TooManyUiContributions,
    #[error("artifact persistence contract must declare a sha256 schema digest")]
    InvalidPersistenceContract,
    #[error("forked artifact slug must remain `{expected}`, received `{received}`")]
    ForkSlugMismatch { expected: String, received: String },
    #[error("forked artifact version must be newer than `{parent}`, received `{received}`")]
    ForkVersionNotIncremented { parent: String, received: String },
    #[error("forked artifact source and payload digests must both change")]
    ForkDigestNotChanged,
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

/// Validates an admitted event subscription. A terminal wildcard is permitted
/// only for subscriptions; delivered platform event types must be exact.
pub(crate) fn valid_event_topic(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 || value == "*" {
        return false;
    }
    let segments = value.split('.').collect::<Vec<_>>();
    segments.iter().enumerate().all(|(index, segment)| {
        if *segment == "*" {
            return index + 1 == segments.len();
        }
        !segment.is_empty()
            && segment.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || character == '_'
                    || character == '-'
            })
    })
}

fn valid_localization_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment.len() <= 48
                && segment
                    .bytes()
                    .enumerate()
                    .all(|(index, byte)| match index {
                        0 => byte.is_ascii_lowercase(),
                        _ => byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_',
                    })
        })
}

fn valid_localization_message(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_LOCALIZATION_MESSAGE_BYTES
        && value
            .chars()
            .all(|character| !character.is_control() && !matches!(character, '<' | '>'))
}

fn valid_ui_route(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'/')
        })
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

/// Matches one admitted exact or terminal-wildcard subscription against an
/// exact platform event type. Callers must validate the delivered event type
/// separately; wildcard syntax is never valid in a delivered envelope.
pub(crate) fn event_topic_matches(subscription: &str, event_type: &str) -> bool {
    subscription == event_type
        || subscription
            .strip_suffix(".*")
            .is_some_and(|prefix| event_type.starts_with(&format!("{prefix}.")))
}

impl ModuleScheduleBinding {
    fn validate(&self) -> bool {
        schedule_cron_expression(&self.cron)
            .is_some_and(|expression| Schedule::from_str(&expression).is_ok())
            && Tz::from_str(&self.timezone).is_ok()
    }
}

/// Converts the admitted five-field minute cron form to the six-field parser
/// form with an explicit zero-second prefix. Six-field descriptors already
/// carry `second minute hour day month weekday` directly.
pub(crate) fn schedule_cron_expression(value: &str) -> Option<String> {
    let fields = value.split_whitespace().collect::<Vec<_>>();
    if !matches!(fields.len(), 5 | 6)
        || fields.iter().any(|field| {
            field.is_empty()
                || !field.chars().all(|character| {
                    character.is_ascii_alphanumeric()
                        || matches!(character, '*' | '/' | ',' | '-' | '?' | '#')
                })
        })
    {
        return None;
    }
    Some(match fields.len() {
        5 => format!("0 {value}"),
        6 => value.to_string(),
        _ => unreachable!("field count is already validated"),
    })
}

/// Canonical immutable identity for a declared schedule. Durable slot records
/// store this digest so a descriptor replacement cannot silently complete or
/// deduplicate work for a different cron/timezone/policy contract.
pub fn schedule_binding_digest(schedule: &ModuleScheduleBinding) -> String {
    format!(
        "sha256:{}",
        hash_manifest_snapshot(&serde_json::json!({
            "cron": schedule.cron,
            "timezone": schedule.timezone,
            "misfire": schedule.misfire,
            "overlap": schedule.overlap,
            "deduplication": schedule.deduplication,
        }))
    )
}

impl ModuleHttpBinding {
    fn validate(&self) -> bool {
        self.request_media_type == "application/json"
            && self.response_media_type == "application/json"
            && self.max_body_bytes > 0
            && self.max_body_bytes <= 1_048_576
            && self.max_output_bytes > 0
            && self.max_output_bytes <= 1_048_576
            && self.timeout_ms > 0
            && self.timeout_ms <= 60_000
            && matches!(self.streaming, ModuleHttpStreamingPolicy::Forbidden)
            && valid_http_relative_path(&self.path)
    }
}

fn valid_http_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.starts_with('/')
        && value.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || matches!(character, '_' | '-')
                })
        })
}

fn validate_local_schema_references(schema: &Value) -> Result<(), ModuleArtifactError> {
    match schema {
        Value::Object(object) => {
            for key in ["$ref", "$dynamicRef", "$recursiveRef"] {
                if let Some(Value::String(reference)) = object.get(key)
                    && !reference.starts_with('#')
                {
                    return Err(ModuleArtifactError::NonLocalSchemaReference(
                        reference.clone(),
                    ));
                }
            }
            for value in object.values() {
                validate_local_schema_references(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_local_schema_references(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Canonical digest used by descriptor schema selectors and binding contracts.
pub fn canonical_schema_digest(schema: &Value) -> String {
    format!("sha256:{}", hash_manifest_snapshot(schema))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permission_localization(label: &str) -> ArtifactPermissionLocalization {
        ArtifactPermissionLocalization {
            locale: "en".to_string(),
            label: label.to_string(),
            description: format!("{label} permission"),
        }
    }

    fn digest(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn schema_document(title: &str) -> ArtifactSchemaDocument {
        let document = serde_json::json!({
            "$schema": JSON_SCHEMA_DRAFT_2020_12,
            "title": title,
            "type": "object"
        });
        ArtifactSchemaDocument {
            digest: canonical_schema_digest(&document),
            document,
        }
    }

    fn localization_catalog(messages: &[(&str, &str)]) -> ArtifactLocalizationCatalog {
        let messages = BTreeMap::from([(
            "en".to_string(),
            messages
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect(),
        )]);
        ArtifactLocalizationCatalog {
            digest: canonical_schema_digest(
                &serde_json::to_value(&messages).expect("localization catalog serializes"),
            ),
            messages,
        }
    }

    fn descriptor(
        kind: ArtifactPayloadKind,
        version: &str,
        marker: char,
    ) -> ModuleArtifactDescriptor {
        ModuleArtifactDescriptor {
            schema_version: MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            slug: "sample_module".to_string(),
            version: version.to_string(),
            payload_kind: kind,
            module_kind: ArtifactModuleKind::Optional,
            runtime_abi: "rustok:module/runtime@1".to_string(),
            platform_compatibility: "^0.1".to_string(),
            required_features: Vec::new(),
            artifact_digest: digest(marker),
            entrypoint: "main".to_string(),
            capabilities: vec![CapabilityName::new("platform.events").expect("capability")],
            bindings: Vec::new(),
            dependencies: Vec::new(),
            permissions: Vec::new(),
            schema_documents: Vec::new(),
            settings_schema_digest: None,
            data_schema_digest: None,
            localization_catalogs: Vec::new(),
            ui_contributions: Vec::new(),
            persistence_contract: None,
        }
    }

    #[test]
    fn source_manifest_omits_and_worker_finalizes_the_component_digest() {
        let mut declaration = descriptor(ArtifactPayloadKind::WasmComponent, "1.0.0", 'a');
        declaration.artifact_digest.clear();
        declaration.entrypoint = "run".to_string();
        let bytes = serde_json::to_vec(&declaration).expect("serialize source manifest");
        assert!(!String::from_utf8_lossy(&bytes).contains("artifact_digest"));

        let source = ModuleArtifactSourceManifest::parse(&bytes).expect("valid source manifest");
        let finalized = source.finalize(digest('b')).expect("finalized descriptor");

        assert_eq!(finalized.artifact_digest, digest('b'));
        assert!(finalized.validate().is_ok());
    }

    #[test]
    fn source_manifest_rejects_an_author_supplied_component_digest() {
        let mut declaration = descriptor(ArtifactPayloadKind::WasmComponent, "1.0.0", 'a');
        declaration.entrypoint = "run".to_string();
        let bytes = serde_json::to_vec(&declaration).expect("serialize descriptor");

        assert!(matches!(
            ModuleArtifactSourceManifest::parse(&bytes),
            Err(ModuleArtifactSourceManifestError::BuildDerivedDigest)
        ));
    }

    #[test]
    fn sandboxed_payloads_select_the_common_executor_registry() {
        assert_eq!(
            descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a')
                .payload_kind
                .sandbox_executor(),
            Some(SandboxExecutorKind::Rhai)
        );
        assert_eq!(
            descriptor(ArtifactPayloadKind::WasmComponent, "1.0.0", 'b')
                .payload_kind
                .sandbox_executor(),
            Some(SandboxExecutorKind::WasmComponent)
        );
        assert_eq!(
            descriptor(ArtifactPayloadKind::StaticPromoted, "1.0.0", 'c')
                .payload_kind
                .sandbox_executor(),
            None
        );
    }

    #[test]
    fn alloy_fork_creates_a_new_immutable_release_lineage() {
        let original = ArtifactReleaseDraft {
            descriptor: descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a'),
            lineage: ArtifactSourceLineage {
                origin: ArtifactOrigin::AlloyDraft,
                source_digest: digest('1'),
                parent_release: None,
            },
        }
        .publish(Utc::now())
        .expect("publish original");

        let fork = original
            .fork(
                descriptor(ArtifactPayloadKind::Rhai, "1.1.0", 'b'),
                digest('2'),
            )
            .expect("fork")
            .publish(Utc::now())
            .expect("publish fork");

        assert_eq!(
            fork.lineage.parent_release,
            Some(original.descriptor.release_ref())
        );
        assert_eq!(original.descriptor.version, "1.0.0");
        assert_eq!(fork.descriptor.version, "1.1.0");
    }

    #[test]
    fn duplicate_capabilities_are_rejected_before_publish() {
        let capability = CapabilityName::new("platform.events").expect("capability");
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        descriptor.capabilities.push(capability);

        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::DuplicateCapability(_))
        ));
    }

    #[test]
    fn binding_cannot_expand_descriptor_capabilities() {
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        descriptor.permissions = vec![ArtifactPermissionDescriptor {
            key: "sample_module.lifecycle.manage".to_string(),
            localizations: vec![permission_localization("Manage lifecycle")],
        }];
        descriptor.bindings.push(ModuleRuntimeBinding {
            id: "pre_enable".to_string(),
            kind: ModuleRuntimeBindingKind::PreEnable,
            entrypoint: "pre_enable".to_string(),
            input_schema_digest: digest('b'),
            output_schema_digest: digest('c'),
            permission: "sample_module.lifecycle.manage".to_string(),
            idempotency: ModuleBindingIdempotency::Required,
            limit_profile: "lifecycle".to_string(),
            capabilities: vec![CapabilityName::new("platform.http").expect("capability")],
            event_topics: Vec::new(),
            schedule: None,
            http: None,
        });

        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::UndeclaredBindingCapability(_))
        ));
    }

    #[test]
    fn dependencies_require_unique_non_self_semver_constraints() {
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        descriptor.dependencies = vec![ModuleDependencyConstraint {
            slug: "sample_module".to_string(),
            version_requirement: "^1".to_string(),
        }];
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::InvalidDependency(_))
        ));

        descriptor.dependencies = vec![ModuleDependencyConstraint {
            slug: "base_module".to_string(),
            version_requirement: "not-a-version".to_string(),
        }];
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::InvalidDependencyVersionRequirement { .. })
        ));
    }

    #[test]
    fn descriptor_requires_supported_schema_and_platform_compatibility() {
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        descriptor.schema_version = 1;
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::UnsupportedSchemaVersion(1))
        ));

        descriptor.schema_version = MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION;
        descriptor.platform_compatibility = "invalid".to_string();
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::InvalidPlatformCompatibility(_))
        ));
    }

    #[test]
    fn permissions_must_stay_in_the_artifact_namespace() {
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        descriptor.permissions = vec![ArtifactPermissionDescriptor {
            key: "platform.admin".to_string(),
            localizations: vec![permission_localization("Admin")],
        }];
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::InvalidPermission(_))
        ));
    }

    #[test]
    fn schemas_reject_network_file_and_dynamic_references() {
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        let document = serde_json::json!({
            "$schema": JSON_SCHEMA_DRAFT_2020_12,
            "$ref": "https://schemas.example/settings.json"
        });
        descriptor.schema_documents = vec![ArtifactSchemaDocument {
            digest: canonical_schema_digest(&document),
            document,
        }];
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::NonLocalSchemaReference(_))
        ));

        let document = serde_json::json!({
            "$schema": JSON_SCHEMA_DRAFT_2020_12,
            "$dynamicRef": "file:///tmp/untrusted-schema.json"
        });
        descriptor.schema_documents = vec![ArtifactSchemaDocument {
            digest: canonical_schema_digest(&document),
            document,
        }];
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::NonLocalSchemaReference(_))
        ));
    }

    #[test]
    fn schema_bundle_requires_canonical_digests_and_declared_binding_documents() {
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        descriptor.permissions = vec![ArtifactPermissionDescriptor {
            key: "sample_module.command.execute".to_string(),
            localizations: vec![permission_localization("Execute command")],
        }];
        descriptor.bindings = vec![ModuleRuntimeBinding {
            id: "command".to_string(),
            kind: ModuleRuntimeBindingKind::Command,
            entrypoint: "command".to_string(),
            input_schema_digest: digest('b'),
            output_schema_digest: digest('c'),
            permission: "sample_module.command.execute".to_string(),
            idempotency: ModuleBindingIdempotency::Required,
            limit_profile: "command".to_string(),
            capabilities: Vec::new(),
            event_topics: Vec::new(),
            schedule: None,
            http: None,
        }];
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::MissingSchemaDocument(_))
        ));

        let input = schema_document("command_input");
        let output = schema_document("command_output");
        descriptor.bindings[0].input_schema_digest = input.digest.clone();
        descriptor.bindings[0].output_schema_digest = output.digest.clone();
        descriptor.schema_documents = vec![input.clone(), output];
        assert!(descriptor.validate().is_ok());

        descriptor.schema_documents[0].document["title"] = Value::String("changed".to_string());
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::SchemaDigestMismatch { .. })
        ));
    }

    #[test]
    fn schema_digest_is_stable_across_object_key_order() {
        let left = serde_json::json!({
            "$schema": JSON_SCHEMA_DRAFT_2020_12,
            "type": "object",
            "properties": { "name": { "type": "string" } }
        });
        let right = serde_json::json!({
            "properties": { "name": { "type": "string" } },
            "type": "object",
            "$schema": JSON_SCHEMA_DRAFT_2020_12
        });
        assert_eq!(
            canonical_schema_digest(&left),
            canonical_schema_digest(&right)
        );
    }

    #[test]
    fn persistence_contract_requires_a_positive_revision() {
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        descriptor.persistence_contract = Some(ArtifactPersistenceContract {
            revision: 0,
            schema_digest: digest('b'),
            indexes: Vec::new(),
        });

        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::InvalidPersistenceContract)
        ));
    }

    #[test]
    fn descriptor_rejects_unknown_persistence_or_migration_fields() {
        let mut descriptor_value =
            serde_json::to_value(descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a'))
                .expect("descriptor serializes");
        descriptor_value
            .as_object_mut()
            .expect("descriptor is an object")
            .insert(
                "migrations".to_string(),
                serde_json::json!([{"sql": "DROP TABLE module_artifact_data"}]),
            );
        assert!(serde_json::from_value::<ModuleArtifactDescriptor>(descriptor_value).is_err());

        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        descriptor.persistence_contract = Some(ArtifactPersistenceContract {
            revision: 1,
            schema_digest: digest('b'),
            indexes: Vec::new(),
        });
        let mut persistence_value =
            serde_json::to_value(descriptor).expect("descriptor serializes");
        persistence_value
            .get_mut("persistence_contract")
            .and_then(Value::as_object_mut)
            .expect("persistence contract is an object")
            .insert("bucket".to_string(), serde_json::json!("untrusted-path"));
        assert!(serde_json::from_value::<ModuleArtifactDescriptor>(persistence_value).is_err());
    }

    #[test]
    fn ui_contribution_requires_module_owned_permission() {
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        let catalog = localization_catalog(&[("status.title", "Settings")]);
        descriptor.localization_catalogs = vec![catalog.clone()];
        descriptor.ui_contributions = vec![ArtifactUiContribution {
            id: "settings".to_string(),
            surface: ArtifactUiSurface::AdminStatus,
            localization_digest: catalog.digest,
            permission: "other_module.settings.manage".to_string(),
            content: ArtifactUiContributionContent::Status {
                title_key: "status.title".to_string(),
                status_key: "status.title".to_string(),
            },
        }];
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::InvalidUiContribution(_))
        ));
    }

    #[test]
    fn ui_contributions_allow_only_declarative_host_surfaces() {
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        descriptor.permissions = vec![ArtifactPermissionDescriptor {
            key: "sample_module.settings.manage".to_string(),
            localizations: vec![permission_localization("Manage settings")],
        }];
        let catalog = localization_catalog(&[("status.title", "Status")]);
        descriptor.localization_catalogs = vec![catalog.clone()];
        descriptor.ui_contributions = vec![ArtifactUiContribution {
            id: "custom".to_string(),
            surface: ArtifactUiSurface::AdminSettings,
            localization_digest: catalog.digest,
            permission: "sample_module.settings.manage".to_string(),
            content: ArtifactUiContributionContent::Status {
                title_key: "status.title".to_string(),
                status_key: "status.title".to_string(),
            },
        }];
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::InvalidUiContribution(_))
        ));

        descriptor.ui_contributions[0].surface = ArtifactUiSurface::AdminStatus;
        let mut descriptor_value = serde_json::to_value(descriptor).expect("descriptor serializes");
        descriptor_value
            .get_mut("ui_contributions")
            .and_then(Value::as_array_mut)
            .and_then(|contributions| contributions.first_mut())
            .and_then(Value::as_object_mut)
            .expect("UI contribution is an object")
            .insert(
                "iframe_url".to_string(),
                serde_json::json!("https://untrusted.example/module-ui"),
            );
        assert!(serde_json::from_value::<ModuleArtifactDescriptor>(descriptor_value).is_err());
    }

    #[test]
    fn declarative_actions_bind_exact_admitted_command_contracts() {
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        let input = schema_document("command_input");
        let output = schema_document("command_output");
        descriptor.permissions = vec![ArtifactPermissionDescriptor {
            key: "sample_module.commands.execute".to_string(),
            localizations: vec![permission_localization("Execute command")],
        }];
        descriptor.bindings = vec![ModuleRuntimeBinding {
            id: "execute".to_string(),
            kind: ModuleRuntimeBindingKind::Command,
            entrypoint: "commands.execute".to_string(),
            input_schema_digest: input.digest.clone(),
            output_schema_digest: output.digest.clone(),
            permission: "sample_module.commands.execute".to_string(),
            idempotency: ModuleBindingIdempotency::Required,
            limit_profile: "command".to_string(),
            capabilities: Vec::new(),
            event_topics: Vec::new(),
            schedule: None,
            http: None,
        }];
        descriptor.schema_documents = vec![input, output];
        let catalog = localization_catalog(&[("action.execute", "Execute")]);
        descriptor.localization_catalogs = vec![catalog.clone()];
        descriptor.ui_contributions = vec![ArtifactUiContribution {
            id: "execute".to_string(),
            surface: ArtifactUiSurface::AdminActions,
            localization_digest: catalog.digest.clone(),
            permission: "sample_module.commands.execute".to_string(),
            content: ArtifactUiContributionContent::Action {
                title_key: "action.execute".to_string(),
                binding_id: "execute".to_string(),
                confirmation: ArtifactUiActionConfirmation::Destructive,
                destructive: true,
                audit_policy: ArtifactUiAuditPolicy::Required,
            },
        }];

        descriptor.validate().expect("valid declarative action");
        assert_eq!(catalog.message("en", "action.execute"), Some("Execute"));
        assert_eq!(catalog.message("ru", "action.execute"), None);

        descriptor.bindings[0].idempotency = ModuleBindingIdempotency::BestEffort;
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::InvalidUiContribution(_))
        ));

        descriptor.bindings[0].idempotency = ModuleBindingIdempotency::Required;
        let ArtifactUiContributionContent::Action { confirmation, .. } =
            &mut descriptor.ui_contributions[0].content
        else {
            panic!("fixture must contain an action contribution");
        };
        *confirmation = ArtifactUiActionConfirmation::Acknowledge;
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::InvalidUiContribution(_))
        ));
    }

    #[test]
    fn ui_contribution_schema_and_catalog_reject_unsafe_or_partial_contracts() {
        let catalog = localization_catalog(&[
            ("help.title", "Help"),
            ("help.body", "Use the module controls."),
        ]);
        let contribution = ArtifactUiContribution {
            id: "help".to_string(),
            surface: ArtifactUiSurface::AdminHelp,
            localization_digest: catalog.digest.clone(),
            permission: "sample_module.settings.manage".to_string(),
            content: ArtifactUiContributionContent::Help {
                title_key: "help.title".to_string(),
                body_key: "help.body".to_string(),
            },
        };
        let mut encoded = serde_json::to_value(&contribution).expect("contribution serializes");
        encoded["content"]["iframe_url"] =
            Value::String("https://untrusted.example/module-ui".to_string());
        assert!(!validates_ui_contribution(&encoded));

        let mut partial_catalog = catalog.clone();
        partial_catalog.messages.insert(
            "ru".to_string(),
            BTreeMap::from([("help.title".to_string(), "Help".to_string())]),
        );
        partial_catalog.digest = canonical_schema_digest(
            &serde_json::to_value(&partial_catalog.messages)
                .expect("localization catalog serializes"),
        );
        assert!(!partial_catalog.validate());

        let mut unsafe_catalog = catalog;
        unsafe_catalog
            .messages
            .get_mut("en")
            .expect("English catalog")
            .insert(
                "help.body".to_string(),
                "<script>unsafe</script>".to_string(),
            );
        unsafe_catalog.digest = canonical_schema_digest(
            &serde_json::to_value(&unsafe_catalog.messages)
                .expect("localization catalog serializes"),
        );
        assert!(!unsafe_catalog.validate());
    }

    #[test]
    fn bindings_and_ui_must_reference_declared_permissions() {
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        descriptor.bindings.push(ModuleRuntimeBinding {
            id: "command".to_string(),
            kind: ModuleRuntimeBindingKind::Command,
            entrypoint: "command".to_string(),
            input_schema_digest: digest('b'),
            output_schema_digest: digest('c'),
            permission: "sample_module.commands.execute".to_string(),
            idempotency: ModuleBindingIdempotency::Required,
            limit_profile: "command".to_string(),
            capabilities: Vec::new(),
            event_topics: Vec::new(),
            schedule: None,
            http: None,
        });
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::InvalidBinding(_))
        ));

        descriptor.bindings.clear();
        descriptor.permissions = vec![ArtifactPermissionDescriptor {
            key: "sample_module.settings.manage".to_string(),
            localizations: vec![permission_localization("Manage settings")],
        }];
        let catalog = localization_catalog(&[("status.title", "Settings")]);
        descriptor.localization_catalogs = vec![catalog.clone()];
        descriptor.ui_contributions = vec![ArtifactUiContribution {
            id: "settings".to_string(),
            surface: ArtifactUiSurface::AdminStatus,
            localization_digest: catalog.digest,
            permission: "sample_module.settings.read".to_string(),
            content: ArtifactUiContributionContent::Status {
                title_key: "status.title".to_string(),
                status_key: "status.title".to_string(),
            },
        }];
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::InvalidUiContribution(_))
        ));
    }

    #[test]
    fn http_bindings_are_json_only_relative_routes_with_unique_methods() {
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        descriptor.permissions = vec![ArtifactPermissionDescriptor {
            key: "sample_module.http.status.read".to_string(),
            localizations: vec![permission_localization("Read status")],
        }];
        let input_schema = schema_document("http_input");
        let output_schema = schema_document("http_output");
        let binding = ModuleRuntimeBinding {
            id: "http_status".to_string(),
            kind: ModuleRuntimeBindingKind::Http,
            entrypoint: "http.status".to_string(),
            input_schema_digest: input_schema.digest.clone(),
            output_schema_digest: output_schema.digest.clone(),
            permission: "sample_module.http.status.read".to_string(),
            idempotency: ModuleBindingIdempotency::Required,
            limit_profile: "http_json".to_string(),
            capabilities: Vec::new(),
            event_topics: Vec::new(),
            schedule: None,
            http: Some(ModuleHttpBinding {
                method: ModuleHttpMethod::Get,
                path: "status/summary".to_string(),
                request_media_type: "application/json".to_string(),
                response_media_type: "application/json".to_string(),
                max_body_bytes: 4_096,
                max_output_bytes: 16_384,
                timeout_ms: 5_000,
                streaming: ModuleHttpStreamingPolicy::Forbidden,
            }),
        };
        descriptor.bindings.push(binding.clone());
        descriptor.schema_documents = vec![input_schema, output_schema];
        assert!(descriptor.validate().is_ok());

        let mut duplicate = binding;
        duplicate.id = "http_status_duplicate".to_string();
        descriptor.bindings.push(duplicate);
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::InvalidBinding(_))
        ));

        descriptor.bindings.truncate(1);
        descriptor.bindings[0]
            .http
            .as_mut()
            .expect("HTTP contract")
            .path = "/outside-the-platform-route".to_string();
        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::InvalidBinding(_))
        ));
    }

    #[test]
    fn fork_must_keep_slug_increment_version_and_change_digests() {
        let original = ArtifactReleaseDraft {
            descriptor: descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a'),
            lineage: ArtifactSourceLineage {
                origin: ArtifactOrigin::AlloyDraft,
                source_digest: digest('1'),
                parent_release: None,
            },
        }
        .publish(Utc::now())
        .expect("publish original");

        let mut renamed = descriptor(ArtifactPayloadKind::Rhai, "1.1.0", 'b');
        renamed.slug = "another_module".to_string();
        assert!(matches!(
            original.fork(renamed, digest('2')),
            Err(ModuleArtifactError::ForkSlugMismatch { .. })
        ));

        assert!(matches!(
            original.fork(
                descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'c'),
                digest('3')
            ),
            Err(ModuleArtifactError::ForkVersionNotIncremented { .. })
        ));
        assert!(matches!(
            original.fork(
                descriptor(ArtifactPayloadKind::Rhai, "1.1.0", 'a'),
                digest('2')
            ),
            Err(ModuleArtifactError::ForkDigestNotChanged)
        ));
        assert!(matches!(
            original.fork(
                descriptor(ArtifactPayloadKind::Rhai, "1.1.0", 'b'),
                digest('1')
            ),
            Err(ModuleArtifactError::ForkDigestNotChanged)
        ));
    }
}
