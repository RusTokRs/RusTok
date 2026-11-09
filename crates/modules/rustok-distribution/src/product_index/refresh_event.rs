use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    DomainError, EntityName, ModuleName, SchemaRef, SchemaVersion, register_index_mutation_event,
};
use rustok_product::{
    PRODUCT_INDEX_LOCALE_REFRESH_REQUESTED_EVENT_TYPE,
    PRODUCT_INDEX_VARIANT_REFRESH_REQUESTED_EVENT_TYPE,
};

use super::{
    PRODUCT_SCHEMA_ROUTING_KEY, product::PRODUCT_INDEX_SOURCE,
    variant::PRODUCT_VARIANT_INDEX_SOURCE,
};

const PRODUCT_OWNER_MODULE: &str = "product";
const PRODUCT_VARIANT_SCHEMA_VERSION: u32 = 2;

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    if !extensions.contains::<rustok_product::ProductRuntimeSelected>() {
        return Ok(());
    }

    register_index_mutation_event(
        extensions,
        PRODUCT_OWNER_MODULE,
        PRODUCT_INDEX_LOCALE_REFRESH_REQUESTED_EVENT_TYPE,
        PRODUCT_INDEX_SOURCE,
        product_schema_ref().map_err(invalid_contract)?,
    )
    .map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected Product Index locale refresh route registration failed: {error}"
        ))
    })?;

    register_index_mutation_event(
        extensions,
        PRODUCT_OWNER_MODULE,
        PRODUCT_INDEX_VARIANT_REFRESH_REQUESTED_EVENT_TYPE,
        PRODUCT_VARIANT_INDEX_SOURCE,
        product_variant_schema_ref().map_err(invalid_contract)?,
    )
    .map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected ProductVariant Index refresh route registration failed: {error}"
        ))
    })
}

fn product_schema_ref() -> Result<SchemaRef, DomainError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")?,
        entity: EntityName::new("product")?,
        version: SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY),
    })
}

fn product_variant_schema_ref() -> Result<SchemaRef, DomainError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")?,
        entity: EntityName::new("product_variant")?,
        version: SchemaVersion::new(PRODUCT_VARIANT_SCHEMA_VERSION),
    })
}

fn invalid_contract(error: DomainError) -> rustok_core::Error {
    rustok_core::Error::Validation(format!(
        "selected Product Index refresh contract is invalid: {error}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_index::IndexMutationEventCatalog;

    #[test]
    fn selected_product_registers_exact_locale_and_variant_refresh_routes() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(rustok_product::ProductRuntimeSelected);

        register(&mut extensions).unwrap();

        let routes = extensions
            .get::<IndexMutationEventCatalog>()
            .expect("Product selection must publish refresh event routes");
        assert_eq!(routes.len(), 2);

        let locale = routes
            .get(PRODUCT_INDEX_LOCALE_REFRESH_REQUESTED_EVENT_TYPE)
            .expect("locale refresh route");
        assert_eq!(locale.owner_module(), PRODUCT_OWNER_MODULE);
        assert_eq!(locale.source_name(), PRODUCT_INDEX_SOURCE);
        assert_eq!(locale.schema(), &product_schema_ref().unwrap());

        let variant = routes
            .get(PRODUCT_INDEX_VARIANT_REFRESH_REQUESTED_EVENT_TYPE)
            .expect("variant refresh route");
        assert_eq!(variant.owner_module(), PRODUCT_OWNER_MODULE);
        assert_eq!(variant.source_name(), PRODUCT_VARIANT_INDEX_SOURCE);
        assert_eq!(variant.schema(), &product_variant_schema_ref().unwrap());
    }
}
