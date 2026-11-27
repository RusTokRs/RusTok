mod consumer;
mod source;

use rustok_api::{is_valid_module_slug, localized_index::LocalizedIndexSources};
use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    DomainError, EntityName, FieldCardinality, FieldName, IndexField, IndexSchema, IndexValueType,
    LocaleMode, ModuleName, SchemaRef, SchemaVersion, register_index_schema_source,
    register_postgres_index_source_factory,
};

pub(super) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    let Some(sources) = extensions.get::<LocalizedIndexSources>() else {
        return Ok(());
    };
    for source in sources.sources().to_vec() {
        if !is_valid_module_slug(source.module_slug) || !is_valid_module_slug(source.entity_type) {
            return Err(invalid("invalid source module or entity"));
        }
        let schema = schema(source.module_slug, source.entity_type).map_err(invalid)?;
        let reference = schema.reference.clone();
        let name = format!("{}-localized-records", source.module_slug);
        register_index_schema_source(extensions, source.module_slug, schema).map_err(invalid)?;
        register_postgres_index_source_factory(
            extensions,
            source.module_slug,
            name.clone(),
            source::Factory {
                owner: source,
                reference,
                name,
            },
        )
        .map_err(invalid)?;
    }
    Ok(())
}

fn schema(slug: &str, entity: &str) -> Result<IndexSchema, DomainError> {
    let schema = IndexSchema {
        reference: SchemaRef {
            module: ModuleName::new(format!("rustok-{}", slug.replace('_', "-")))?,
            entity: EntityName::new(entity)?,
            version: SchemaVersion::INITIAL,
        },
        locale_mode: LocaleMode::Required,
        fields: [
            ("id", IndexValueType::Uuid),
            ("title", IndexValueType::String),
        ]
        .into_iter()
        .map(|(name, value_type)| {
            Ok(IndexField {
                name: FieldName::new(name)?,
                value_type,
                cardinality: FieldCardinality::One,
                nullable: false,
                selectable: true,
                filterable: true,
                sortable: true,
            })
        })
        .collect::<Result<_, DomainError>>()?,
        links: Vec::new(),
    };
    schema.validate()?;
    Ok(schema)
}

fn invalid(error: impl std::fmt::Display) -> rustok_core::Error {
    rustok_core::Error::Validation(format!(
        "Localized record Index registration failed: {error}"
    ))
}
