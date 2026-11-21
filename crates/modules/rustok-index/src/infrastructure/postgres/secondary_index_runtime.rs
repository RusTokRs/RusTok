use std::time::Duration;

use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{FieldName, SchemaRef, SharedIndexSchemaRegistry};

use super::{
    IndexJobOperatorAuthority, PostgresSecondaryIndexManager, SecondaryIndexError,
    SecondaryIndexExecutionOutcome, SecondaryIndexOperation, SecondaryIndexPlan,
    SecondaryIndexRequest,
};

/// Explicit operator maintenance of one selected schema field. Module composition does not start
/// DDL or create a per-tenant index fleet. The request tenant comes only from operator authority.
#[derive(Clone)]
pub struct SharedIndexSecondaryIndexRuntime {
    manager: PostgresSecondaryIndexManager,
    schemas: SharedIndexSchemaRegistry,
}

impl SharedIndexSecondaryIndexRuntime {
    pub(super) fn new(db: DatabaseConnection, schemas: SharedIndexSchemaRegistry) -> Self {
        Self {
            manager: PostgresSecondaryIndexManager::new(db),
            schemas,
        }
    }

    pub async fn maintain(
        &self,
        authority: IndexJobOperatorAuthority,
        schema: &SchemaRef,
        field: &FieldName,
        operation: SecondaryIndexOperation,
    ) -> Result<Option<SecondaryIndexExecutionOutcome>, SecondaryIndexError> {
        let schema = self
            .schemas
            .registry()
            .get(schema)
            .ok_or_else(|| SecondaryIndexError::SchemaNotSelected(schema.clone()))?;
        let plan = SecondaryIndexPlan::from_schema(authority.tenant_id(), &schema.schema)?;
        let spec = plan
            .indexes()
            .iter()
            .find(|spec| spec.field_name() == field)
            .ok_or(SecondaryIndexError::UnsupportedField)?;
        let request = SecondaryIndexRequest::new(
            spec.clone(),
            operation,
            format!("index-ddl-{}", Uuid::new_v4().simple()),
            Duration::from_secs(90),
        )?;
        self.manager.operate(&request).await
    }
}
