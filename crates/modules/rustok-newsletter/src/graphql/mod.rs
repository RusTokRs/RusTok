//! GraphQL adapters for the newsletter module.

mod mutation;
mod query;
mod types;

pub use mutation::NewsletterMutation;
pub use query::NewsletterQuery;

use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// Runtime data attached to the GraphQL schema for newsletter resolvers.
pub struct NewsletterGraphqlRuntimeData {
    pub db: DatabaseConnection,
}

impl NewsletterGraphqlRuntimeData {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

/// Error type for newsletter GraphQL runtime initialization.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct NewsletterGraphqlRuntimeError(pub String);

impl From<&str> for NewsletterGraphqlRuntimeError {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for NewsletterGraphqlRuntimeError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<NewsletterGraphqlRuntimeError> for String {
    fn from(e: NewsletterGraphqlRuntimeError) -> Self {
        e.0
    }
}

/// Factory function to attach newsletter runtime data to the GraphQL context.
pub fn attach_schema_data(
    inputs: &rustok_api::graphql::GraphqlRuntimeInputs,
) -> Result<Arc<NewsletterGraphqlRuntimeData>, NewsletterGraphqlRuntimeError> {
    Ok(Arc::new(NewsletterGraphqlRuntimeData::new(
        inputs.db_clone(),
    )))
}
