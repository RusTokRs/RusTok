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

/// Factory function to attach newsletter runtime data to the GraphQL context.
pub fn attach_schema_data(
    inputs: &rustok_api::graphql::GraphqlRuntimeInputs,
) -> Result<Arc<NewsletterGraphqlRuntimeData>, String> {
    Ok(Arc::new(NewsletterGraphqlRuntimeData::new(
        inputs.db_clone(),
    )))
}
