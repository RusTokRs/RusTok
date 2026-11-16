use flex_ports::{
    FlexAttachedSchema, FlexVersionedAttachedSchema, FlexVersionedAttachedSchemaReadPort,
};
use rustok_api::{PortContext, PortError};
use rustok_core::{
    error::Error as CoreError,
    field_schema::{CustomFieldsSchema, is_valid_field_key},
};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TransactionTrait};
use serde_json::Value;
use uuid::Uuid;

use super::catalog::helpers::load_product_custom_fields_schema;
use crate::CommerceResult;

/// Product's durable copy of the latest authoritative Flex schema for each tenant.
pub struct ProductFlexSchemaProjection {
    db: DatabaseConnection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductFlexSchemaApplyOutcome {
    Advanced,
    Replay,
    Stale,
}

impl ProductFlexSchemaProjection {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn refresh(
        &self,
        context: PortContext,
        source: &dyn FlexVersionedAttachedSchemaReadPort,
    ) -> Result<ProductFlexSchemaApplyOutcome, PortError> {
        context.require_read_semantics()?;
        let tenant_id = Uuid::parse_str(&context.tenant_id)
            .ok()
            .filter(|id| !id.is_nil())
            .ok_or_else(|| {
                PortError::validation("product.tenant_id_invalid", "Product tenant is invalid")
            })?;
        let snapshot = source
            .read_versioned_attached_schema(context, flex::PRODUCT_ENTITY_TYPE)
            .await?;
        self.apply(tenant_id, snapshot).await
    }

    /// Apply a full snapshot from the authenticated Flex source. All decisions for one tenant
    /// share the row lock acquired by the conditional upsert.
    pub async fn apply(
        &self,
        tenant_id: Uuid,
        snapshot: FlexVersionedAttachedSchema,
    ) -> Result<ProductFlexSchemaApplyOutcome, PortError> {
        use ProductFlexSchemaApplyOutcome::{Advanced, Replay, Stale};

        snapshot.verify_scope(tenant_id, flex::PRODUCT_ENTITY_TYPE)?;
        decode_definitions(&snapshot.schema)?;
        let payload = serde_json::to_value(&snapshot.schema).map_err(|_| invalid_projection())?;
        let txn = self
            .db
            .begin()
            .await
            .map_err(|_| projection_unavailable())?;
        let result = txn.execute_raw(Statement::from_sql_and_values(
            txn.get_database_backend(),
            "INSERT INTO product_flex_schema_projections (tenant_id, revision, schema_json) VALUES ($1, $2, $3)
             ON CONFLICT (tenant_id) DO UPDATE SET revision = excluded.revision, schema_json = excluded.schema_json
             WHERE product_flex_schema_projections.revision < excluded.revision",
            [tenant_id.into(), snapshot.revision.into(), payload.clone().into()],
        )).await.map_err(|_| projection_unavailable())?;
        let outcome = if result.rows_affected() == 1 {
            Advanced
        } else {
            let (revision, existing) = projected_schema(&txn, tenant_id)
                .await?
                .ok_or_else(invalid_projection)?;
            if revision > snapshot.revision {
                Stale
            } else if revision == snapshot.revision && existing == payload {
                Replay
            } else {
                return Err(PortError::invariant_violation(
                    "product.flex_schema_revision_conflict",
                    "Flex schema revision conflicts with the stored snapshot",
                ));
            }
        };
        txn.commit().await.map_err(|_| projection_unavailable())?;
        Ok(outcome)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductFlexSchemaSource {
    Shared,
    Projection,
}

impl ProductFlexSchemaSource {
    pub(crate) async fn load(
        self,
        db: &impl ConnectionTrait,
        tenant_id: Uuid,
    ) -> CommerceResult<CustomFieldsSchema> {
        match self {
            Self::Shared => load_product_custom_fields_schema(db, tenant_id).await,
            Self::Projection => {
                let (revision, payload) = projected_schema(db, tenant_id)
                    .await
                    .map_err(projected_schema_error)?
                    .ok_or_else(|| {
                        CoreError::External("Product Flex schema projection is not seeded".into())
                    })?;
                let schema = serde_json::from_value(payload)
                    .map_err(|_| projected_schema_error(invalid_projection()))?;
                let snapshot = FlexVersionedAttachedSchema { revision, schema };
                snapshot
                    .verify_scope(tenant_id, flex::PRODUCT_ENTITY_TYPE)
                    .map_err(projected_schema_error)?;
                decode_definitions(&snapshot.schema)
                    .map_err(|error| projected_schema_error(error).into())
            }
        }
    }
}

async fn projected_schema(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<Option<(i64, Value)>, PortError> {
    let row = db.query_one_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "SELECT revision, schema_json FROM product_flex_schema_projections WHERE tenant_id = $1",
        [tenant_id.into()],
    )).await.map_err(|_| projection_unavailable())?;
    row.map(|row| {
        Ok((
            row.try_get("", "revision")
                .map_err(|_| invalid_projection())?,
            row.try_get("", "schema_json")
                .map_err(|_| invalid_projection())?,
        ))
    })
    .transpose()
}

fn decode_definitions(schema: &FlexAttachedSchema) -> Result<CustomFieldsSchema, PortError> {
    if schema
        .definitions
        .iter()
        .any(|definition| !is_valid_field_key(&definition.field_key))
    {
        return Err(invalid_projection());
    }
    let definitions = schema
        .definitions
        .iter()
        .map(flex::field_definition_from_source)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid_projection())?;
    Ok(CustomFieldsSchema::new(definitions))
}

fn projection_unavailable() -> PortError {
    PortError::unavailable(
        "product.flex_schema_projection_unavailable",
        "Product Flex schema projection is temporarily unavailable",
    )
}

fn invalid_projection() -> PortError {
    PortError::invariant_violation(
        "product.flex_schema_projection_invalid",
        "Product Flex schema projection is invalid",
    )
}

fn projected_schema_error(error: PortError) -> CoreError {
    if error.retryable {
        CoreError::External(error.message)
    } else {
        CoreError::Internal(error.message)
    }
}
