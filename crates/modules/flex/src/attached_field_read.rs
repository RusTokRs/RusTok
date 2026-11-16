use std::collections::BTreeMap;

use async_trait::async_trait;
use flex_ports::{
    FlexAttachedFieldPolicyReadPort, FlexAttachedSchema, FlexAttachedSchemaReadPort,
    FlexVersionedAttachedSchema, FlexVersionedAttachedSchemaReadPort,
};
use rustok_api::{PortContext, PortError};
use rustok_core::field_schema::{FlexError, is_valid_field_key};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use uuid::Uuid;

use crate::{
    FieldDefRegistry, FlexAttachedFieldPolicyError, FlexAttachedFieldPolicyResolution,
    MAX_GENERIC_ATTACHED_FIELDS_PER_TENANT, field_definition_from_source, list_field_definitions,
    resolve_attached_field_policy_resolutions,
};

/// Embedded Flex provider over the registered definition stores and the policy store.
pub struct FlexAttachedFieldReadService {
    db: DatabaseConnection,
    registry: FieldDefRegistry,
}

impl FlexAttachedFieldReadService {
    pub fn new(db: DatabaseConnection, registry: FieldDefRegistry) -> Self {
        Self { db, registry }
    }

    async fn product_schema_revision(&self, tenant_id: Uuid) -> Result<i64, PortError> {
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                self.db.get_database_backend(),
                "SELECT revision FROM flex_product_attached_schema_revisions WHERE tenant_id = $1",
                [tenant_id.into()],
            ))
            .await
            .map_err(|_| schema_revision_unavailable())?;
        row.map(|row| {
            row.try_get("", "revision")
                .map_err(|_| schema_revision_unavailable())
        })
        .transpose()
        .map(|revision| revision.unwrap_or_default())
    }
}

#[async_trait]
impl FlexVersionedAttachedSchemaReadPort for FlexAttachedFieldReadService {
    async fn read_versioned_attached_schema(
        &self,
        context: PortContext,
        entity_type: &str,
    ) -> Result<FlexVersionedAttachedSchema, PortError> {
        let tenant_id = attached_read_tenant(&context)?;
        if entity_type != crate::PRODUCT_ENTITY_TYPE {
            return Err(PortError::unavailable(
                "flex.versioned_attached_schema_unsupported",
                "Versioned Flex schema is unavailable for this donor",
            ));
        }
        // The legacy registry uses a connection pool. The transactional revision fence
        // confirms that no definitions committed between these independent reads.
        let revision = self.product_schema_revision(tenant_id).await?;
        let schema = self.read_attached_schema(context, entity_type).await?;
        if self.product_schema_revision(tenant_id).await? != revision {
            return Err(PortError::unavailable(
                "flex.attached_schema_changed",
                "Flex attached schema changed during the read; retry",
            ));
        }
        let snapshot = FlexVersionedAttachedSchema { revision, schema };
        snapshot.verify_scope(tenant_id, entity_type)?;
        Ok(snapshot)
    }
}

fn schema_revision_unavailable() -> PortError {
    PortError::unavailable(
        "flex.attached_schema_revision_unavailable",
        "Flex attached schema revision is temporarily unavailable",
    )
}

#[async_trait]
impl FlexAttachedSchemaReadPort for FlexAttachedFieldReadService {
    async fn read_attached_schema(
        &self,
        context: PortContext,
        entity_type: &str,
    ) -> Result<FlexAttachedSchema, PortError> {
        let tenant_id = attached_read_tenant(&context)?;
        let mut definitions =
            list_field_definitions(&self.registry, &self.db, tenant_id, entity_type)
                .await
                .map_err(schema_error)?;
        definitions.retain(|definition| definition.is_active);
        definitions.sort_by_key(|definition| (definition.position, definition.id));
        let schema = FlexAttachedSchema {
            tenant_id,
            entity_type: entity_type.to_owned(),
            definitions,
        };
        schema.verify_scope(tenant_id, entity_type)?;
        if schema.definitions.iter().any(|definition| {
            !is_valid_field_key(&definition.field_key)
                || field_definition_from_source(definition).is_err()
        }) {
            return Err(PortError::invariant_violation(
                "flex.attached_schema_invalid",
                "Flex attached schema could not be read safely",
            ));
        }
        Ok(schema)
    }
}

#[async_trait]
impl FlexAttachedFieldPolicyReadPort for FlexAttachedFieldReadService {
    async fn read_attached_field_policies(
        &self,
        context: PortContext,
        entity_type: &str,
        field_keys: &[String],
    ) -> Result<BTreeMap<String, FlexAttachedFieldPolicyResolution>, PortError> {
        let tenant_id = attached_read_tenant(&context)?;
        self.registry.get(entity_type).map_err(schema_error)?;
        if field_keys.len() > MAX_GENERIC_ATTACHED_FIELDS_PER_TENANT
            || field_keys.iter().any(|key| !is_valid_field_key(key))
        {
            return Err(PortError::validation(
                "flex.attached_policy_scope_invalid",
                "Flex attached policy field selection is invalid",
            ));
        }
        resolve_attached_field_policy_resolutions(&self.db, tenant_id, entity_type, field_keys)
            .await
            .map_err(|error| match error {
                FlexAttachedFieldPolicyError::Storage(_) => PortError::unavailable(
                    "flex.attached_policy_unavailable",
                    "Flex attached policy is temporarily unavailable",
                ),
                FlexAttachedFieldPolicyError::Invalid(_) => PortError::invariant_violation(
                    "flex.attached_policy_invalid",
                    "Flex attached policy could not be read safely",
                ),
            })
    }
}

fn attached_read_tenant(context: &PortContext) -> Result<Uuid, PortError> {
    context.require_read_semantics()?;
    Uuid::parse_str(&context.tenant_id)
        .ok()
        .filter(|tenant| !tenant.is_nil())
        .ok_or_else(|| PortError::validation("flex.tenant_id_invalid", "Flex tenant is invalid"))
}

fn schema_error(error: FlexError) -> PortError {
    match error {
        FlexError::UnknownEntityType(_) => PortError::validation(
            "flex.attached_donor_unknown",
            "Flex donor type is not registered",
        ),
        FlexError::Database(_) => PortError::unavailable(
            "flex.attached_schema_unavailable",
            "Flex attached schema is temporarily unavailable",
        ),
        _ => PortError::invariant_violation(
            "flex.attached_schema_invalid",
            "Flex attached schema could not be read safely",
        ),
    }
}
