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
    #[error("project does not contain a mutable root component")]
    MissingProjectRoot,
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
    #[error("registry item `{0}` is already registered")]
    DuplicateRegistryItem(String),
    #[error("registry item id `{0}` must be namespaced or one of the built-in ids")]
    InvalidRegistryId(String),
    #[error("plugin dependency `{dependency}` required by `{plugin}` is missing")]
    MissingPluginDependency { plugin: String, dependency: String },
    #[error("plugin dependency cycle contains `{0}`")]
    PluginDependencyCycle(String),
    #[error("command would move `{component}` inside its own subtree through `{parent}`")]
    RecursiveMove { component: String, parent: String },
    #[error("project validation failed")]
    Validation(Vec<ValidationDiagnostic>),
    #[error("undo history is empty")]
    UndoHistoryEmpty,
    #[error("redo history is empty")]
    RedoHistoryEmpty,
    #[error("revision conflict: expected `{expected}`, current `{actual}`")]
    RevisionConflict { expected: String, actual: String },
}
