use rustok_index::{
    DomainError, EntityName, FieldCardinality, FieldName, IndexField, IndexSchema, IndexValueType,
    LocaleMode, ModuleName, SchemaRef, SchemaVersion,
};

pub(super) fn schema_ref() -> Result<SchemaRef, DomainError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-inventory")?,
        entity: EntityName::new("stock")?,
        version: SchemaVersion::INITIAL,
    })
}

pub(super) fn stock_schema() -> Result<IndexSchema, DomainError> {
    let mut fields = Vec::new();
    for name in ["id", "product_id", "inventory_item_id", "baseline_id"] {
        fields.push(field(name, IndexValueType::Uuid)?);
    }
    for name in ["stocked", "reserved", "incoming", "available"] {
        fields.push(field(name, IndexValueType::Integer)?);
    }
    let schema = IndexSchema {
        reference: schema_ref()?,
        locale_mode: LocaleMode::None,
        fields,
        links: Vec::new(),
    };
    schema.validate()?;
    Ok(schema)
}

fn field(name: &str, value_type: IndexValueType) -> Result<IndexField, DomainError> {
    Ok(IndexField {
        name: FieldName::new(name)?,
        value_type,
        cardinality: FieldCardinality::One,
        nullable: false,
        selectable: true,
        filterable: true,
        sortable: true,
    })
}
