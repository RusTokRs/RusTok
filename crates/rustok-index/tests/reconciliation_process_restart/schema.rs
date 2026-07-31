use std::collections::BTreeMap;

use rustok_index::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexMutation, IndexRecord,
    IndexSchema, IndexValue, IndexValueType, LocaleMode, ModuleName, SchemaRef, SchemaVersion,
};
use uuid::Uuid;

pub const SOURCE_NAME: &str = "process-restart-primary";

pub fn schema_ref() -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("process-restart").unwrap(),
        entity: EntityName::new("item").unwrap(),
        version: SchemaVersion::INITIAL,
    }
}

pub fn schema() -> IndexSchema {
    IndexSchema {
        reference: schema_ref(),
        locale_mode: LocaleMode::None,
        fields: vec![IndexField {
            name: FieldName::new("id").unwrap(),
            value_type: IndexValueType::Uuid,
            cardinality: FieldCardinality::One,
            nullable: false,
            selectable: true,
            filterable: true,
            sortable: true,
        }],
        links: Vec::new(),
    }
}

pub fn mutation(tenant_id: Uuid, id: u128) -> IndexMutation {
    let entity_id = Uuid::from_u128(id);
    IndexMutation::Upsert {
        event_id: Uuid::from_u128(20_000 + id),
        record: IndexRecord {
            key: EntityKey {
                tenant_id,
                schema: schema_ref(),
                entity_id,
                locale: None,
            },
            source_version: 1,
            fields: BTreeMap::from([(
                FieldName::new("id").unwrap(),
                IndexValue::Uuid(entity_id),
            )]),
            links: Vec::new(),
        },
    }
}
