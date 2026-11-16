//! Portable read contracts for Flex attached field definitions and governance.

mod definitions;
mod policy;

use std::collections::{BTreeMap, HashSet};

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use definitions::{FieldDefinitionView, FieldDefinitionViewSource};
pub use policy::{
    FlexAttachedFieldPolicy, FlexAttachedFieldPolicyError, FlexAttachedFieldPolicyResolution,
    FlexAttachedFieldPolicyResult, FlexDataClassification,
};

pub const MAX_FLEX_ATTACHED_FIELDS: usize = 50;

/// The active definitions for one tenant and donor type at the time of the read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlexAttachedSchema {
    pub tenant_id: Uuid,
    pub entity_type: String,
    pub definitions: Vec<FieldDefinitionView>,
}

impl FlexAttachedSchema {
    pub fn verify_scope(&self, tenant_id: Uuid, entity_type: &str) -> Result<(), PortError> {
        let mut keys = HashSet::with_capacity(self.definitions.len().min(MAX_FLEX_ATTACHED_FIELDS));
        if self.tenant_id != tenant_id
            || self.entity_type != entity_type
            || self.definitions.len() > MAX_FLEX_ATTACHED_FIELDS
            || self
                .definitions
                .iter()
                .any(|definition| !definition.is_active || !keys.insert(&definition.field_key))
        {
            return Err(PortError::invariant_violation(
                "flex.attached_schema_invalid",
                "Flex attached schema could not be read safely",
            ));
        }
        Ok(())
    }
}

#[async_trait]
pub trait FlexAttachedSchemaReadPort: Send + Sync {
    async fn read_attached_schema(
        &self,
        context: PortContext,
        entity_type: &str,
    ) -> Result<FlexAttachedSchema, PortError>;
}

/// Complete authoritative snapshot and its monotonic revision for the tenant and donor.
/// Revision zero is the initial snapshot before any tracked edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlexVersionedAttachedSchema {
    pub revision: i64,
    pub schema: FlexAttachedSchema,
}

impl FlexVersionedAttachedSchema {
    pub fn verify_scope(&self, tenant_id: Uuid, entity_type: &str) -> Result<(), PortError> {
        if self.revision < 0 || tenant_id.is_nil() {
            return Err(PortError::invariant_violation(
                "flex.attached_schema_revision_invalid",
                "Flex attached schema revision is invalid",
            ));
        }
        self.schema.verify_scope(tenant_id, entity_type)
    }
}

#[async_trait]
pub trait FlexVersionedAttachedSchemaReadPort: Send + Sync {
    async fn read_versioned_attached_schema(
        &self,
        context: PortContext,
        entity_type: &str,
    ) -> Result<FlexVersionedAttachedSchema, PortError>;
}

#[async_trait]
pub trait FlexAttachedFieldPolicyReadPort: Send + Sync {
    /// Returns every requested field. Defaults and explicit policies are distinguishable.
    async fn read_attached_field_policies(
        &self,
        context: PortContext,
        entity_type: &str,
        field_keys: &[String],
    ) -> Result<BTreeMap<String, FlexAttachedFieldPolicyResolution>, PortError>;
}
