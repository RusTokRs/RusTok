use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum FormError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Field error: {field} - {message}")]
    Field { field: String, message: String },

    #[error("Submit error: {0}")]
    Submit(String),
}

impl FormError {
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn field(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Field {
            field: field.into(),
            message: message.into(),
        }
    }

    pub fn submit(msg: impl Into<String>) -> Self {
        Self::Submit(msg.into())
    }
}
