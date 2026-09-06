use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CartCoreError {
    Validation(String),
}

impl Display for CartCoreError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CartCoreError {}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{}: {}", context, error)
}
