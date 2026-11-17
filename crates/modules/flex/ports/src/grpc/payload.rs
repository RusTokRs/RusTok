use rustok_api::PortError;

use super::proto;
#[cfg(feature = "grpc-client")]
use super::tenant_uuid;
use crate::{FieldDefinitionView, FlexVersionedAttachedSchema};

#[cfg(feature = "grpc-server")]
pub(super) fn encode(
    snapshot: FlexVersionedAttachedSchema,
) -> Result<proto::AttachedSchemaSnapshot, PortError> {
    Ok(proto::AttachedSchemaSnapshot {
        tenant_id: snapshot.schema.tenant_id.to_string(),
        entity_type: snapshot.schema.entity_type,
        revision: snapshot.revision,
        definitions: snapshot
            .schema
            .definitions
            .into_iter()
            .map(encode_field)
            .collect::<Result<_, _>>()?,
    })
}

#[cfg(feature = "grpc-server")]
fn encode_field(field: FieldDefinitionView) -> Result<proto::FieldDefinition, PortError> {
    let json =
        |value: serde_json::Value| serde_json::to_vec(&value).map_err(|_| invalid_snapshot());
    Ok(proto::FieldDefinition {
        id: field.id.to_string(),
        field_key: field.field_key,
        field_type: field.field_type,
        label_json: json(field.label)?,
        description_json: field.description.map(json).transpose()?,
        is_localized: field.is_localized,
        is_required: field.is_required,
        default_value_json: field.default_value.map(json).transpose()?,
        validation_json: field.validation.map(json).transpose()?,
        position: field.position,
        is_active: field.is_active,
        created_at: field.created_at,
        updated_at: field.updated_at,
    })
}

#[cfg(feature = "grpc-client")]
pub(super) fn decode(
    snapshot: proto::AttachedSchemaSnapshot,
) -> Result<FlexVersionedAttachedSchema, PortError> {
    if snapshot.definitions.len() > crate::MAX_FLEX_ATTACHED_FIELDS {
        return Err(invalid_snapshot());
    }
    Ok(FlexVersionedAttachedSchema {
        revision: snapshot.revision,
        schema: crate::FlexAttachedSchema {
            tenant_id: tenant_uuid(&snapshot.tenant_id).ok_or_else(invalid_snapshot)?,
            entity_type: snapshot.entity_type,
            definitions: snapshot
                .definitions
                .into_iter()
                .map(decode_field)
                .collect::<Result<_, _>>()?,
        },
    })
}

#[cfg(feature = "grpc-client")]
fn decode_field(field: proto::FieldDefinition) -> Result<FieldDefinitionView, PortError> {
    let json = |bytes: &[u8]| serde_json::from_slice(bytes).map_err(|_| invalid_snapshot());
    Ok(FieldDefinitionView {
        id: tenant_uuid(&field.id).ok_or_else(invalid_snapshot)?,
        field_key: field.field_key,
        field_type: field.field_type,
        label: json(&field.label_json)?,
        description: field.description_json.as_deref().map(json).transpose()?,
        is_localized: field.is_localized,
        is_required: field.is_required,
        default_value: field.default_value_json.as_deref().map(json).transpose()?,
        validation: field.validation_json.as_deref().map(json).transpose()?,
        position: field.position,
        is_active: field.is_active,
        created_at: field.created_at,
        updated_at: field.updated_at,
    })
}

fn invalid_snapshot() -> PortError {
    PortError::invariant_violation(
        "flex.grpc_snapshot_invalid",
        "Flex gRPC schema snapshot is invalid",
    )
}
