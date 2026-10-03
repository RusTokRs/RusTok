use crate::ValidationDiagnostic;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum FlyError {
    #[error("failed to decode GrapesJS project: {0}")]
    Decode(String),
    #[error("failed to encode GrapesJS project: {0}")]
    Encode(String),
    #[error("project root must be a JSON object")]
    InvalidProjectRoot,
    #[error("project bundle is invalid: {0}")]
    InvalidProjectBundle(String),
    #[error("project bundle hash mismatch: declared `{declared}`, actual `{actual}`")]
    ProjectBundleHashMismatch { declared: String, actual: String },
    #[error("project does not contain a mutable root component")]
    MissingProjectRoot,
    #[error("page `{0}` does not contain a renderable root component")]
    MissingPageRoot(String),
    #[error("component `{0}` was not found")]
    ComponentNotFound(String),
    #[error("parent component `{0}` was not found")]
    ParentNotFound(String),
    #[error("component `{0}` is opaque and cannot be edited by Fly")]
    OpaqueComponent(String),
    #[error("component insertion index {index} is outside 0..={len}")]
    InvalidInsertionIndex { index: usize, len: usize },
    #[error("page `{0}` was not found")]
    PageNotFound(String),
    #[error("page locator must contain an id or index")]
    InvalidPageLocator,
    #[error("page index {index} is outside 0..={len}")]
    InvalidPageIndex { index: usize, len: usize },
    #[error("page id `{0}` is duplicated")]
    DuplicatePageId(String),
    #[error("the last page cannot be removed")]
    LastPageRemoval,
    #[error("asset `{0}` was not found")]
    AssetNotFound(String),
    #[error("asset reference is invalid: {0}")]
    InvalidAssetReference(String),
    #[error("style rule `{0}` was not found")]
    StyleRuleNotFound(String),
    #[error("trait `{trait_id}` value is invalid: {message}")]
    InvalidTraitValue { trait_id: String, message: String },
    #[error("component patch is invalid: {0}")]
    InvalidComponentPatch(String),
    #[error("page patch is invalid: {0}")]
    InvalidPagePatch(String),
    #[error("registry item `{0}` is already registered")]
    DuplicateRegistryItem(String),
    #[error("registry item id `{0}` must be namespaced or one of the built-in ids")]
    InvalidRegistryId(String),
    #[error("interaction capability contract is invalid: {0}")]
    InvalidInteractionCapability(String),
    #[error("plugin dependency `{dependency}` required by `{plugin}` is missing")]
    MissingPluginDependency { plugin: String, dependency: String },
    #[error("plugin dependency cycle contains `{0}`")]
    PluginDependencyCycle(String),
    #[error("command would move `{component}` inside its own subtree through `{parent}`")]
    RecursiveMove { component: String, parent: String },
    #[error("project validation failed: {}", format_diagnostics(.0))]
    Validation(Vec<ValidationDiagnostic>),
    #[error("snapshot `{0}` was not found")]
    SnapshotNotFound(String),
    #[error("snapshot `{snapshot_id}` hash mismatch: declared `{declared}`, restored `{actual}`")]
    SnapshotHashMismatch {
        snapshot_id: String,
        declared: String,
        actual: String,
    },
    #[error(
        "snapshot `{snapshot_id}` content digest mismatch: declared `{declared}`, actual `{actual}`"
    )]
    SnapshotDigestMismatch {
        snapshot_id: String,
        declared: String,
        actual: String,
    },
    #[error("snapshot `{0}` has no content digest and cannot be verified")]
    SnapshotDigestMissing(String),
    #[error("project bundle content digest mismatch: declared `{declared}`, actual `{actual}`")]
    ProjectBundleDigestMismatch { declared: String, actual: String },
    #[error("project bundle has no content digest and cannot be verified")]
    ProjectBundleDigestMissing,
    #[error("component id `{id}` is not a valid identifier: {reason}")]
    InvalidComponentId { id: String, reason: String },
    #[error("project nesting depth exceeds the maximum of {maximum}")]
    MaximumDepthExceeded { maximum: usize },
    #[error("undo history is empty")]
    UndoHistoryEmpty,
    #[error("redo history is empty")]
    RedoHistoryEmpty,
    #[error("revision conflict: expected `{expected}`, current `{actual}`")]
    RevisionConflict { expected: String, actual: String },
}

/// Render validation diagnostics into a single human-readable line.
///
/// `FlyError::Validation` used to render as a bare "project validation failed", which meant every
/// log line and every transport error dropped the one thing a caller needs: which rule failed and
/// where. The list is truncated so a pathological document cannot produce an unbounded message.
fn format_diagnostics(diagnostics: &[ValidationDiagnostic]) -> String {
    const MAXIMUM_RENDERED: usize = 5;

    if diagnostics.is_empty() {
        return "no diagnostics were reported".to_string();
    }
    let rendered = diagnostics
        .iter()
        .take(MAXIMUM_RENDERED)
        .map(|diagnostic| {
            format!(
                "{} at `{}`: {}",
                diagnostic.code, diagnostic.path, diagnostic.message
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    if diagnostics.len() > MAXIMUM_RENDERED {
        format!(
            "{rendered}; and {} more",
            diagnostics.len() - MAXIMUM_RENDERED
        )
    } else {
        rendered
    }
}
