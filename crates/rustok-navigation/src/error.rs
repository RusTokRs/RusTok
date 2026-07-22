use rustok_core::error::{Error as CoreError, ErrorKind, RichError};
use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum NavigationError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
    #[error("Core error: {0}")]
    Core(#[from] CoreError),
    #[error("Menu not found: {0}")]
    MenuNotFound(Uuid),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Rich error: {0}")]
    Rich(#[from] Box<RichError>),
}

pub type NavigationResult<T> = Result<T, NavigationError>;

impl NavigationError {
    pub fn menu_not_found(menu_id: Uuid) -> Self { Self::MenuNotFound(menu_id) }
    pub fn validation(message: impl Into<String>) -> Self { Self::Validation(message.into()) }
    pub fn forbidden(message: impl Into<String>) -> Self { Self::Forbidden(message.into()) }
}

impl From<NavigationError> for RichError {
    fn from(error: NavigationError) -> Self {
        match error {
            NavigationError::Database(source) => RichError::new(ErrorKind::Database, "Navigation database operation failed")
                .with_user_message("Unable to access navigation data")
                .with_source(source),
            NavigationError::Core(source) => source.into(),
            NavigationError::MenuNotFound(id) => RichError::new(ErrorKind::NotFound, format!("Menu {id} not found"))
                .with_user_message("The requested menu does not exist")
                .with_field("menu_id", id.to_string())
                .with_error_code("MENU_NOT_FOUND"),
            NavigationError::Validation(message) => RichError::new(ErrorKind::Validation, message)
                .with_user_message("Invalid navigation input"),
            NavigationError::Forbidden(message) => RichError::new(ErrorKind::Forbidden, message)
                .with_user_message("You do not have permission to manage navigation"),
            NavigationError::Rich(error) => *error,
        }
    }
}
