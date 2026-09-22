use std::sync::Arc;

use async_graphql::{Context, FieldError, Result as GraphqlResult};
use async_trait::async_trait;
use rustok_api::graphql::GraphQLError;
use rustok_core::field_schema::FlexError;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::{FieldDefRegistry, FieldDefinitionCachePort, FlexStandaloneService};

/// Host-owned attached-value operations exposed through the generic Flex GraphQL transport.
///
/// Flex owns the transport/schema contract, while the application host remains responsible for
/// validating the concrete donor identity before generic attached rows are read or written.
#[async_trait]
pub trait AttachedValuesGraphqlPort: Send + Sync {
    async fn resolve_values(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        preferred_locale: &str,
        tenant_default_locale: &str,
    ) -> std::result::Result<Option<Value>, FlexError>;

    async fn update_values(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        locale: &str,
        payload: Option<Value>,
    ) -> std::result::Result<Option<Value>, FlexError>;

    async fn delete_values(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> std::result::Result<(), FlexError>;
}

pub struct FlexGraphqlRuntime {
    standalone_service: Arc<dyn FlexStandaloneService>,
    db: DatabaseConnection,
    field_registry: FieldDefRegistry,
    field_definition_cache: Arc<dyn FieldDefinitionCachePort>,
    attached_values: Arc<dyn AttachedValuesGraphqlPort>,
}

impl FlexGraphqlRuntime {
    pub fn new(
        standalone_service: Arc<dyn FlexStandaloneService>,
        db: DatabaseConnection,
        field_registry: FieldDefRegistry,
        field_definition_cache: Arc<dyn FieldDefinitionCachePort>,
        attached_values: Arc<dyn AttachedValuesGraphqlPort>,
    ) -> Self {
        Self {
            standalone_service,
            db,
            field_registry,
            field_definition_cache,
            attached_values,
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

    pub(crate) fn attached_values(&self) -> &dyn AttachedValuesGraphqlPort {
        self.attached_values.as_ref()
    }
}

pub(crate) fn runtime<'ctx>(ctx: &'ctx Context<'_>) -> GraphqlResult<&'ctx FlexGraphqlRuntime> {
    ctx.data::<FlexGraphqlRuntime>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "FlexGraphqlRuntime is not registered; initialize the Flex host adapter",
        )
    })
}
