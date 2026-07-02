use std::sync::Arc;

use async_graphql::{Context, FieldError, Result};
use rustok_api::graphql::GraphQLError;
use sea_orm::DatabaseConnection;

use crate::{FieldDefRegistry, FieldDefinitionCachePort, FlexStandaloneService};

pub struct FlexGraphqlRuntime {
    standalone_service: Arc<dyn FlexStandaloneService>,
    db: DatabaseConnection,
    field_registry: FieldDefRegistry,
    field_definition_cache: Arc<dyn FieldDefinitionCachePort>,
}

impl FlexGraphqlRuntime {
    pub fn new(
        standalone_service: Arc<dyn FlexStandaloneService>,
        db: DatabaseConnection,
        field_registry: FieldDefRegistry,
        field_definition_cache: Arc<dyn FieldDefinitionCachePort>,
    ) -> Self {
        Self {
            standalone_service,
            db,
            field_registry,
            field_definition_cache,
        }
    }

    pub(crate) fn standalone_service(&self) -> Arc<dyn FlexStandaloneService> {
        Arc::clone(&self.standalone_service)
    }

    pub(crate) fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub(crate) fn field_registry(&self) -> &FieldDefRegistry {
        &self.field_registry
    }

    pub(crate) fn field_definition_cache(&self) -> &dyn FieldDefinitionCachePort {
        self.field_definition_cache.as_ref()
    }
}

pub(crate) fn runtime<'ctx>(ctx: &'ctx Context<'_>) -> Result<&'ctx FlexGraphqlRuntime> {
    ctx.data::<FlexGraphqlRuntime>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "FlexGraphqlRuntime is not registered; initialize the Flex host adapter",
        )
    })
}
