use thiserror::Error;

#[derive(Error, Debug)]
pub enum IndexError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Index error: {0}")]
    Index(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type IndexResult<T> = Result<T, IndexError>;

impl From<IndexError> for rustok_core::Error {
    fn from(error: IndexError) -> Self {
        match error {
            IndexError::Database(error) => Self::Database(error),
            IndexError::NotFound { entity_type, id } => {
                Self::NotFound(format!("{entity_type} with id {id}"))
            }
            IndexError::Index(message) => Self::Cache(message),
            IndexError::Serialization(error) => Self::Serialization(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::IndexError;

    #[test]
    fn operational_index_failure_is_not_reported_as_not_found() {
        let error: rustok_core::Error = IndexError::Index("projection write failed".to_string()).into();

        assert!(matches!(error, rustok_core::Error::Cache(message) if message == "projection write failed"));
    }

    #[test]
    fn missing_entity_preserves_not_found_category() {
        let error: rustok_core::Error = IndexError::NotFound {
            entity_type: "product".to_string(),
            id: "42".to_string(),
        }
        .into();

        assert!(matches!(error, rustok_core::Error::NotFound(message) if message == "product with id 42"));
    }
}
