use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum UiError {
    #[error("editor is read-only")]
    ReadOnly,
    #[error("capability `{0}` is unavailable")]
    CapabilityUnavailable(String),
    #[error("contribution `{0}` is already registered")]
    DuplicateContribution(String),
    #[error("contribution `{0}` requires a missing provider")]
    MissingContributionProvider(String),
    #[error("contribution `{contribution}` is invalid: {message}")]
    InvalidContribution {
        contribution: String,
        message: String,
    },
    #[error("renderer contract `{0}` is already registered")]
    DuplicateRenderer(String),
    #[error("property editor contract `{0}` is already registered")]
    DuplicatePropertyEditor(String),
    #[error("renderer `{0}` is unavailable")]
    RendererUnavailable(String),
    #[error("property editor `{0}` is unavailable")]
    PropertyEditorUnavailable(String),
    #[error("adapter rejected the request: {0}")]
    AdapterRejected(String),
    #[error("drop is not legal: {0}")]
    IllegalDrop(String),
    #[error("no drag operation is active")]
    NoActiveDrag,
}
