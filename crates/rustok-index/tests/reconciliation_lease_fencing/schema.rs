use rustok_index::{
    EntityName, FieldCardinality, FieldName, IndexField, IndexSchema, IndexValueType, LocaleMode,
    ModuleName, SchemaRef, SchemaVersion,
};

pub fn schema_ref() -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("lease-fencing-harness").unwrap(),
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
