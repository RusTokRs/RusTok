use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RbacError {
    #[error("permission denied")]
    PermissionDenied,
}
