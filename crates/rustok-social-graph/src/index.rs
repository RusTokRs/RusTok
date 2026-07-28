use std::collections::BTreeMap;

use rustok_events::{EventValidationError, SocialGraphRelationEvent, ValidateEvent};
use rustok_index::{
    DomainError, EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexMutation,
    IndexRecord, IndexSchema, IndexValue, IndexValueType, LocaleMode, ModuleName, SchemaRef,
    SchemaVersion,
};
use thiserror::Error;
use uuid::Uuid;

pub const SOCIAL_GRAPH_RELATION_INDEX_MODULE: &str = "rustok-social-graph";
pub const SOCIAL_GRAPH_RELATION_INDEX_ENTITY: &str = "relation";

const SOURCE_USER_ID_FIELD: &str = "source_user_id";
const TARGET_USER_ID_FIELD: &str = "target_user_id";
const RELATION_KIND_FIELD: &str = "relation_kind";

#[derive(Debug, Error)]
pub enum SocialGraphIndexError {
    #[error("tenant_id cannot be nil")]
    NilTenantId,
    #[error("event_id cannot be nil")]
    NilEventId,
    #[error("invalid social graph relation event: {0}")]
    InvalidEvent(#[from] EventValidationError),
    #[error("invalid social graph Index contract: {0}")]
    InvalidIndexContract(#[from] DomainError),
}

/// Owner-published generic Index schema for active Social Graph relations.
///
/// Inactive revisions are represented as Index tombstones. The relation revision
/// is used as `source_version`, so duplicate and lower revisions are terminally
/// ignored by the Index mutation store while a later reactivation can upsert the
/// same relation id again.
pub fn social_graph_relation_index_schema() -> Result<IndexSchema, SocialGraphIndexError> {
    let schema = IndexSchema {
        reference: relation_schema_ref()?,
        locale_mode: LocaleMode::None,
        fields: vec![
            relation_field(SOURCE_USER_ID_FIELD, IndexValueType::Uuid)?,
            relation_field(TARGET_USER_ID_FIELD, IndexValueType::Uuid)?,
            relation_field(RELATION_KIND_FIELD, IndexValueType::String)?,
        ],
        links: Vec::new(),
    };
    schema.validate()?;
    Ok(schema)
}

/// Converts one validated sealed relation event into the generic Index mutation
/// consumed by an approved durable Index delivery worker.
///
/// The caller must acknowledge the broker delivery only after the Index owner has
/// durably committed or terminally recognized the mutation. Social Graph storage
/// remains authoritative for bounded replay and drift repair.
pub fn social_graph_relation_index_mutation(
    tenant_id: Uuid,
    event_id: Uuid,
    event: SocialGraphRelationEvent,
) -> Result<IndexMutation, SocialGraphIndexError> {
    if tenant_id.is_nil() {
        return Err(SocialGraphIndexError::NilTenantId);
    }
    if event_id.is_nil() {
        return Err(SocialGraphIndexError::NilEventId);
    }
    event.validate()?;

    let SocialGraphRelationEvent::RelationStateChanged {
        relation_id,
        source_user_id,
        target_user_id,
        relation_kind,
        active,
        revision,
    } = event;

    let source_version = u64::try_from(revision).map_err(|_| {
        SocialGraphIndexError::InvalidEvent(EventValidationError::OutOfRange(
            "revision",
            revision,
            1,
            i64::MAX,
        ))
    })?;
    let key = EntityKey {
        tenant_id,
        schema: relation_schema_ref()?,
        entity_id: relation_id,
        locale: None,
    };

    if !active {
        return Ok(IndexMutation::Delete {
            event_id,
            key,
            source_version,
        });
    }

    let fields = BTreeMap::from([
        (
            FieldName::new(SOURCE_USER_ID_FIELD)?,
            IndexValue::Uuid(source_user_id),
        ),
        (
            FieldName::new(TARGET_USER_ID_FIELD)?,
            IndexValue::Uuid(target_user_id),
        ),
        (
            FieldName::new(RELATION_KIND_FIELD)?,
            IndexValue::String(relation_kind),
        ),
    ]);

    Ok(IndexMutation::Upsert {
        event_id,
        record: IndexRecord {
            key,
            source_version,
            fields,
            links: Vec::new(),
        },
    })
}

fn relation_schema_ref() -> Result<SchemaRef, DomainError> {
    Ok(SchemaRef {
        module: ModuleName::new(SOCIAL_GRAPH_RELATION_INDEX_MODULE)?,
        entity: EntityName::new(SOCIAL_GRAPH_RELATION_INDEX_ENTITY)?,
        version: SchemaVersion::INITIAL,
    })
}

fn relation_field(name: &str, value_type: IndexValueType) -> Result<IndexField, DomainError> {
    Ok(IndexField {
        name: FieldName::new(name)?,
        value_type,
        cardinality: FieldCardinality::One,
        nullable: false,
        selectable: true,
        filterable: true,
        sortable: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(active: bool, revision: i64) -> SocialGraphRelationEvent {
        SocialGraphRelationEvent::RelationStateChanged {
            relation_id: Uuid::from_u128(10),
            source_user_id: Uuid::from_u128(11),
            target_user_id: Uuid::from_u128(12),
            relation_kind: "follow".to_string(),
            active,
            revision,
        }
    }

    #[test]
    fn relation_schema_is_valid_and_non_localized() {
        let schema = social_graph_relation_index_schema().unwrap();
        assert_eq!(
            schema.reference.module.as_str(),
            SOCIAL_GRAPH_RELATION_INDEX_MODULE
        );
        assert_eq!(
            schema.reference.entity.as_str(),
            SOCIAL_GRAPH_RELATION_INDEX_ENTITY
        );
        assert_eq!(schema.locale_mode, LocaleMode::None);
        assert_eq!(schema.fields.len(), 3);
        assert!(schema.links.is_empty());
        assert!(schema.fingerprint().is_ok());
    }

    #[test]
    fn active_relation_maps_to_monotonic_upsert() {
        let tenant_id = Uuid::from_u128(1);
        let event_id = Uuid::from_u128(2);
        let mutation =
            social_graph_relation_index_mutation(tenant_id, event_id, event(true, 7)).unwrap();

        let IndexMutation::Upsert {
            event_id: actual_event_id,
            record,
        } = mutation
        else {
            panic!("active relation must map to an upsert");
        };
        assert_eq!(actual_event_id, event_id);
        assert_eq!(record.key.tenant_id, tenant_id);
        assert_eq!(record.key.entity_id, Uuid::from_u128(10));
        assert_eq!(record.source_version, 7);
        assert_eq!(
            record
                .fields
                .get(&FieldName::new(RELATION_KIND_FIELD).unwrap()),
            Some(&IndexValue::String("follow".to_string()))
        );
    }

    #[test]
    fn inactive_relation_maps_to_revisioned_tombstone() {
        let mutation = social_graph_relation_index_mutation(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            event(false, 8),
        )
        .unwrap();

        let IndexMutation::Delete {
            key,
            source_version,
            ..
        } = mutation
        else {
            panic!("inactive relation must map to a tombstone");
        };
        assert_eq!(key.entity_id, Uuid::from_u128(10));
        assert_eq!(source_version, 8);
    }

    #[test]
    fn invalid_envelope_identity_and_revision_fail_closed() {
        assert!(matches!(
            social_graph_relation_index_mutation(Uuid::nil(), Uuid::from_u128(2), event(true, 1)),
            Err(SocialGraphIndexError::NilTenantId)
        ));
        assert!(matches!(
            social_graph_relation_index_mutation(Uuid::from_u128(1), Uuid::nil(), event(true, 1)),
            Err(SocialGraphIndexError::NilEventId)
        ));
        assert!(matches!(
            social_graph_relation_index_mutation(
                Uuid::from_u128(1),
                Uuid::from_u128(2),
                event(true, 0)
            ),
            Err(SocialGraphIndexError::InvalidEvent(_))
        ));
    }
}
