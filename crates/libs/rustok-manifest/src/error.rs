use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("I/O error reading manifest from '{0}': {1}")]
    Io(String, #[source] std::io::Error),

    #[error("Failed to parse TOML from '{0}': {1}")]
    TomlParse(String, #[source] toml::de::Error),

    #[error("Failed to serialize manifest to TOML: {0}")]
    TomlSerialize(#[source] toml::ser::Error),

    #[error("Unsupported manifest schema version: {0} (expected 2)")]
    UnsupportedSchema(u32),

    #[error(
        "Invalid module slug '{0}': slugs must be 1-48 chars of lowercase ascii, digits, or underscore, and not start/end with underscore"
    )]
    InvalidSlug(String),

    #[error("Unknown module slug '{0}' referenced in settings or dependencies")]
    UnknownModule(String),

    #[error("Cyclic dependency detected in module dependency graph: {0}")]
    CyclicDependency(String),

    #[error("Module '{0}' requires missing dependency '{1}'")]
    MissingDependency(String, String),

    #[error("Invalid UI classification '{0}' for module '{1}'")]
    InvalidUiClassification(String, String),

    #[error(
        "Invalid module ownership '{0}' for module '{1}': must be 'first_party' or 'third_party'"
    )]
    InvalidOwnership(String, String),

    #[error(
        "Invalid trust level '{0}' for module '{1}': must be 'core', 'verified', 'unverified', or 'private'"
    )]
    InvalidTrustLevel(String, String),

    #[error("Manifest validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, ManifestError>;
