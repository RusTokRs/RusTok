use async_graphql::{Enum, InputObject, Json, SimpleObject};
use chrono::{DateTime, Utc};
use rustok_seo_targets::SeoTargetSlug;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[graphql(rename_items = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SeoRedirectMatchType {
    Exact,
    Wildcard,
}

impl SeoRedirectMatchType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Wildcard => "wildcard",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "exact" => Some(Self::Exact),
            "wildcard" => Some(Self::Wildcard),
            _ => None,
        }
    }
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoAlternateLink {
    pub locale: String,
    pub href: String,
    pub x_default: bool,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoRedirectDecision {
    pub target_url: String,
    pub status_code: i32,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoRouteContext {
    pub target_kind: Option<SeoTargetSlug>,
    pub target_id: Option<Uuid>,
    pub requested_locale: Option<String>,
    pub effective_locale: String,
    pub canonical_url: String,
    pub redirect: Option<SeoRedirectDecision>,
    pub alternates: Vec<SeoAlternateLink>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoImageAsset {
    pub url: String,
    pub alt: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub mime_type: Option<String>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoOpenGraph {
    pub title: Option<String>,
    pub description: Option<String>,
    pub kind: Option<String>,
    pub site_name: Option<String>,
    pub url: Option<String>,
    pub locale: Option<String>,
    pub images: Vec<SeoImageAsset>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoTwitterCard {
    pub card: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub site: Option<String>,
    pub creator: Option<String>,
    pub images: Vec<SeoImageAsset>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoVerificationTag {
    pub name: String,
    pub value: String,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoVerification {
    pub google: Vec<String>,
    pub yandex: Vec<String>,
    pub yahoo: Vec<String>,
    pub other: Vec<SeoVerificationTag>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoPagination {
    pub prev_url: Option<String>,
    pub next_url: Option<String>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoStructuredDataBlock {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub payload: Json<Value>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoMetaTag {
    pub name: Option<String>,
    pub property: Option<String>,
    pub http_equiv: Option<String>,
    pub content: String,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoLinkTag {
    pub rel: String,
    pub href: String,
    pub hreflang: Option<String>,
    pub media: Option<String>,
    pub mime_type: Option<String>,
    pub title: Option<String>,
}

#[derive(Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[graphql(rename_items = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SeoFieldSource {
    Explicit,
    Generated,
    #[default]
    Fallback,
}

impl SeoFieldSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Generated => "generated",
            Self::Fallback => "fallback",
        }
    }
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoFieldState {
    pub source: SeoFieldSource,
    pub present: bool,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoDocumentEffectiveState {
    pub title: SeoFieldState,
    pub description: SeoFieldState,
    pub canonical_url: SeoFieldState,
    pub keywords: SeoFieldState,
    pub robots: SeoFieldState,
    pub open_graph: SeoFieldState,
    pub twitter: SeoFieldState,
    pub structured_data: SeoFieldState,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoRobots {
    pub index: bool,
    pub follow: bool,
    pub noarchive: bool,
    pub nosnippet: bool,
    pub noimageindex: bool,
    pub notranslate: bool,
    pub max_snippet: Option<i32>,
    pub max_image_preview: Option<String>,
    pub max_video_preview: Option<i32>,
    pub custom: Vec<String>,
}

impl Default for SeoRobots {
    fn default() -> Self {
        Self {
            index: true,
            follow: true,
            noarchive: false,
            nosnippet: false,
            noimageindex: false,
            notranslate: false,
            max_snippet: None,
            max_image_preview: None,
            max_video_preview: None,
            custom: Vec::new(),
        }
    }
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoDocument {
    pub title: String,
    pub description: Option<String>,
    pub robots: SeoRobots,
    pub open_graph: Option<SeoOpenGraph>,
    pub twitter: Option<SeoTwitterCard>,
    pub verification: Option<SeoVerification>,
    pub pagination: Option<SeoPagination>,
    pub structured_data_blocks: Vec<SeoStructuredDataBlock>,
    pub meta_tags: Vec<SeoMetaTag>,
    pub link_tags: Vec<SeoLinkTag>,
    pub effective_state: SeoDocumentEffectiveState,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoPageContext {
    pub route: SeoRouteContext,
    pub document: SeoDocument,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoMetaTranslationInput {
    pub locale: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_image: Option<String>,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoMetaInput {
    pub target_kind: SeoTargetSlug,
    pub target_id: Uuid,
    #[graphql(default)]
    pub noindex: bool,
    #[graphql(default)]
    pub nofollow: bool,
    pub canonical_url: Option<String>,
    pub structured_data: Option<Json<Value>>,
    #[graphql(default)]
    pub translations: Vec<SeoMetaTranslationInput>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoMetaTranslationRecord {
    pub locale: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_image: Option<String>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoMetaRecord {
    pub target_kind: SeoTargetSlug,
    pub target_id: Uuid,
    pub requested_locale: Option<String>,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub noindex: bool,
    pub nofollow: bool,
    pub canonical_url: Option<String>,
    pub translation: SeoMetaTranslationRecord,
    pub source: String,
    pub open_graph: Option<SeoOpenGraph>,
    pub structured_data: Option<Json<Value>>,
    pub effective_state: SeoDocumentEffectiveState,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoRedirectInput {
    pub id: Option<Uuid>,
    pub match_type: SeoRedirectMatchType,
    pub source_pattern: String,
    pub target_url: String,
    pub status_code: i32,
    pub expires_at: Option<DateTime<Utc>>,
    #[graphql(default = true)]
    pub is_active: bool,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoRedirectRecord {
    pub id: Uuid,
    pub match_type: SeoRedirectMatchType,
    pub source_pattern: String,
    pub target_url: String,
    pub status_code: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoRevisionRecord {
    pub id: Uuid,
    pub target_kind: SeoTargetSlug,
    pub target_id: Uuid,
    pub revision: i32,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoSitemapFileRecord {
    pub id: Uuid,
    pub path: String,
    pub url_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoSitemapStatusRecord {
    pub enabled: bool,
    pub latest_job_id: Option<Uuid>,
    pub status: Option<String>,
    pub file_count: i32,
    pub generated_at: Option<DateTime<Utc>>,
    pub files: Vec<SeoSitemapFileRecord>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoRobotsPreviewRecord {
    pub body: String,
    pub public_url: String,
    pub sitemap_index_url: Option<String>,
}

#[derive(Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[graphql(rename_items = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SeoBulkSource {
    Any,
    Explicit,
    Generated,
    Fallback,
}

impl SeoBulkSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Explicit => "explicit",
            Self::Generated => "generated",
            Self::Fallback => "fallback",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "any" => Some(Self::Any),
            "explicit" => Some(Self::Explicit),
            "generated" => Some(Self::Generated),
            "fallback" => Some(Self::Fallback),
            _ => None,
        }
    }
}

#[derive(Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[graphql(rename_items = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SeoBulkSelectionMode {
    SelectedIds,
    CurrentFilterScope,
}

impl SeoBulkSelectionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SelectedIds => "selected_ids",
            Self::CurrentFilterScope => "current_filter_scope",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "selected_ids" => Some(Self::SelectedIds),
            "current_filter_scope" => Some(Self::CurrentFilterScope),
            _ => None,
        }
    }
}

#[derive(Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[graphql(rename_items = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SeoBulkFieldPatchMode {
    Keep,
    Set,
    Clear,
}

impl SeoBulkFieldPatchMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Set => "set",
            Self::Clear => "clear",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "keep" => Some(Self::Keep),
            "set" => Some(Self::Set),
            "clear" => Some(Self::Clear),
            _ => None,
        }
    }
}

#[derive(Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[graphql(rename_items = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SeoBulkJobOperationKind {
    Apply,
    ExportCsv,
    ImportCsv,
}

impl SeoBulkJobOperationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apply => "apply",
            Self::ExportCsv => "export_csv",
            Self::ImportCsv => "import_csv",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "apply" => Some(Self::Apply),
            "export_csv" => Some(Self::ExportCsv),
            "import_csv" => Some(Self::ImportCsv),
            _ => None,
        }
    }
}

#[derive(Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[graphql(rename_items = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SeoBulkJobStatus {
    Queued,
    Running,
    Completed,
    Partial,
    Failed,
}

impl SeoBulkJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Partial => "partial",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "partial" => Some(Self::Partial),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[graphql(rename_items = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SeoBulkApplyMode {
    PreviewOnly,
    #[default]
    ApplyMissingOnly,
    OverwriteGeneratedOnly,
    ForceOverwriteExplicit,
}

impl SeoBulkApplyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PreviewOnly => "preview_only",
            Self::ApplyMissingOnly => "apply_missing_only",
            Self::OverwriteGeneratedOnly => "overwrite_generated_only",
            Self::ForceOverwriteExplicit => "force_overwrite_explicit",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "preview_only" => Some(Self::PreviewOnly),
            "apply_missing_only" => Some(Self::ApplyMissingOnly),
            "overwrite_generated_only" => Some(Self::OverwriteGeneratedOnly),
            "force_overwrite_explicit" => Some(Self::ForceOverwriteExplicit),
            _ => None,
        }
    }
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkListInput {
    pub target_kind: SeoTargetSlug,
    pub locale: String,
    pub query: Option<String>,
    pub source: Option<SeoBulkSource>,
    #[graphql(default = 1)]
    pub page: i32,
    #[graphql(default = 20)]
    pub per_page: i32,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkItem {
    pub target_kind: SeoTargetSlug,
    pub target_id: Uuid,
    pub locale: String,
    pub effective_locale: String,
    pub label: String,
    pub route: String,
    pub source: SeoBulkSource,
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical_url: Option<String>,
    pub noindex: bool,
    pub nofollow: bool,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkPage {
    pub items: Vec<SeoBulkItem>,
    pub total: i32,
    pub page: i32,
    pub per_page: i32,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkSelectionInput {
    pub mode: SeoBulkSelectionMode,
    #[graphql(default)]
    pub selected_ids: Vec<Uuid>,
    pub filter: Option<SeoBulkListInput>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkSelectionPreviewRecord {
    pub count: i32,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkStringFieldPatch {
    pub mode: SeoBulkFieldPatchMode,
    pub value: Option<String>,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkBoolFieldPatch {
    pub mode: SeoBulkFieldPatchMode,
    pub value: Option<bool>,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkJsonFieldPatch {
    pub mode: SeoBulkFieldPatchMode,
    pub value: Option<Json<Value>>,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkMetaPatchInput {
    pub title: Option<SeoBulkStringFieldPatch>,
    pub description: Option<SeoBulkStringFieldPatch>,
    pub keywords: Option<SeoBulkStringFieldPatch>,
    pub canonical_url: Option<SeoBulkStringFieldPatch>,
    pub og_title: Option<SeoBulkStringFieldPatch>,
    pub og_description: Option<SeoBulkStringFieldPatch>,
    pub og_image: Option<SeoBulkStringFieldPatch>,
    pub structured_data: Option<SeoBulkJsonFieldPatch>,
    pub noindex: Option<SeoBulkBoolFieldPatch>,
    pub nofollow: Option<SeoBulkBoolFieldPatch>,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkApplyInput {
    pub selection: SeoBulkSelectionInput,
    pub patch: SeoBulkMetaPatchInput,
    #[graphql(default)]
    #[serde(default)]
    pub apply_mode: SeoBulkApplyMode,
    #[graphql(default = true)]
    pub publish_after_write: bool,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkImportInput {
    pub target_kind: SeoTargetSlug,
    pub locale: String,
    pub csv_utf8: String,
    #[graphql(default = true)]
    pub publish_after_write: bool,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkExportInput {
    pub filter: SeoBulkListInput,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkArtifactRecord {
    pub id: Uuid,
    pub job_id: Uuid,
    pub kind: String,
    pub file_name: String,
    pub mime_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoBulkJobRecord {
    pub id: Uuid,
    pub operation_kind: SeoBulkJobOperationKind,
    pub status: SeoBulkJobStatus,
    pub target_kind: SeoTargetSlug,
    pub locale: String,
    pub publish_after_write: bool,
    pub matched_count: i32,
    pub processed_count: i32,
    pub succeeded_count: i32,
    pub failed_count: i32,
    pub artifact_count: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub artifacts: Vec<SeoBulkArtifactRecord>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SeoModuleSettings {
    #[serde(default = "default_robots")]
    pub default_robots: Vec<String>,
    #[serde(default = "default_sitemap_enabled")]
    pub sitemap_enabled: bool,
    #[serde(default)]
    pub allowed_redirect_hosts: Vec<String>,
    #[serde(default)]
    pub allowed_canonical_hosts: Vec<String>,
    #[serde(default)]
    pub x_default_locale: Option<String>,
    #[serde(default)]
    pub template_defaults: SeoTemplateRuleSet,
    #[serde(default)]
    pub template_overrides: BTreeMap<String, SeoTemplateRuleSet>,
}

impl Default for SeoModuleSettings {
    fn default() -> Self {
        Self {
            default_robots: default_robots(),
            sitemap_enabled: default_sitemap_enabled(),
            allowed_redirect_hosts: Vec::new(),
            allowed_canonical_hosts: Vec::new(),
            x_default_locale: None,
            template_defaults: SeoTemplateRuleSet::default(),
            template_overrides: BTreeMap::new(),
        }
    }
}

#[derive(SimpleObject, InputObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoTemplateRuleSet {
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub canonical_url: Option<String>,
    pub keywords: Option<String>,
    pub robots: Option<String>,
    pub open_graph_title: Option<String>,
    pub open_graph_description: Option<String>,
    pub twitter_title: Option<String>,
    pub twitter_description: Option<String>,
}

#[derive(Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[graphql(rename_items = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SeoDiagnosticSeverity {
    #[default]
    Info,
    Warning,
    Error,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoDiagnosticIssueRecord {
    pub code: String,
    pub severity: SeoDiagnosticSeverity,
    pub target_kind: SeoTargetSlug,
    pub target_id: Uuid,
    pub target_label: String,
    pub route: String,
    pub locale: String,
    pub message: String,
    pub canonical_url: Option<String>,
    pub source: String,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoDiagnosticCountRecord {
    pub key: String,
    pub count: i32,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoDiagnosticsSummaryRecord {
    pub locale: String,
    pub total_targets: i32,
    pub readiness_score: i32,
    pub issue_count: i32,
    pub error_count: i32,
    pub warning_count: i32,
    pub generated_count: i32,
    pub explicit_count: i32,
    pub fallback_count: i32,
    pub issue_counts_by_code: Vec<SeoDiagnosticCountRecord>,
    pub issue_counts_by_target_kind: Vec<SeoDiagnosticCountRecord>,
    pub issues: Vec<SeoDiagnosticIssueRecord>,
}

fn default_robots() -> Vec<String> {
    vec!["index".to_string(), "follow".to_string()]
}

fn default_sitemap_enabled() -> bool {
    true
}
