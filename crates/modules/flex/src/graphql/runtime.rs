use std::sync::Arc;

use async_graphql::{Context, FieldError, Result as GraphqlResult};
use async_trait::async_trait;
use rustok_api::{PortContext, PortError, graphql::GraphQLError};
use rustok_core::field_schema::FlexError;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    FieldDefRegistry, FieldDefinitionCachePort, FlexAttachedFieldPolicyReadPort,
    FlexAttachedFieldReadService, FlexStandaloneService,
};

/// Host-owned attached-value operations exposed through the generic Flex GraphQL transport.
///
/// Flex owns the transport/schema contract, while the application host remains responsible for
/// validating the concrete donor identity before generic attached rows are read or written.
#[async_trait]
pub trait AttachedValuesGraphqlPort: Send + Sync {
    async fn resolve_values(
        &self,
        context: PortContext,
        entity_type: &str,
        entity_id: Uuid,
        tenant_default_locale: &str,
    ) -> std::result::Result<Option<Value>, FlexError>;

    async fn update_values(
        &self,
        context: PortContext,
        entity_type: &str,
        entity_id: Uuid,
        payload: Option<Value>,
    ) -> std::result::Result<Option<Value>, AttachedValuesCommandError>;

    async fn delete_values(
        &self,
        context: PortContext,
        entity_type: &str,
        entity_id: Uuid,
    ) -> std::result::Result<(), AttachedValuesCommandError>;
}

#[derive(Debug)]
pub enum AttachedValuesCommandError {
    Flex(FlexError),
    Owner(PortError),
}

impl std::fmt::Display for AttachedValuesCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Flex(error) => error.fmt(formatter),
            Self::Owner(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AttachedValuesCommandError {}

impl From<FlexError> for AttachedValuesCommandError {
    fn from(error: FlexError) -> Self {
        Self::Flex(error)
    }
}

impl From<PortError> for AttachedValuesCommandError {
    fn from(error: PortError) -> Self {
        Self::Owner(error)
    }
}

pub struct FlexGraphqlRuntime {
    standalone_service: Arc<dyn FlexStandaloneService>,
    db: DatabaseConnection,
    field_registry: FieldDefRegistry,
    field_definition_cache: Arc<dyn FieldDefinitionCachePort>,
    attached_values: Arc<dyn AttachedValuesGraphqlPort>,
    attached_policy_read: FlexAttachedFieldReadService,
}

impl FlexGraphqlRuntime {
    pub fn new(
        standalone_service: Arc<dyn FlexStandaloneService>,
        db: DatabaseConnection,
        field_registry: FieldDefRegistry,
        field_definition_cache: Arc<dyn FieldDefinitionCachePort>,
        attached_values: Arc<dyn AttachedValuesGraphqlPort>,
    ) -> Self {
        let attached_policy_read =
            FlexAttachedFieldReadService::new(db.clone(), field_registry.clone());
        Self {
            standalone_service,
            db,
            field_registry,
            field_definition_cache,
            attached_values,
            attached_policy_read,
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

    pub(crate) fn attached_policy_read(&self) -> &dyn FlexAttachedFieldPolicyReadPort {
        &self.attached_policy_read
    }
}

pub(crate) fn runtime<'ctx>(ctx: &'ctx Context<'_>) -> GraphqlResult<&'ctx FlexGraphqlRuntime> {
    ctx.data::<FlexGraphqlRuntime>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "FlexGraphqlRuntime is not registered; initialize the Flex host adapter",
        )
    })
}
