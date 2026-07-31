pub(crate) mod graph;
#[cfg(test)]
pub(crate) use graph::{PRODUCT_INDEX_SOURCE, PRODUCT_VARIANT_INDEX_SOURCE};
mod product;
#[path = "../product_variant_index.rs"]
mod variant;

pub(crate) fn register(
    extensions: &mut rustok_core::ModuleRuntimeExtensions,
) -> rustok_core::Result<()> {
    product::register(extensions)?;
    variant::register(extensions)
}

#[cfg(test)]
mod tests {
    use rustok_core::ModuleRuntimeExtensions;

    use super::{PRODUCT_INDEX_SOURCE, PRODUCT_VARIANT_INDEX_SOURCE, register};

    #[test]
    fn selected_product_bridge_set_registers_four_schemas_and_two_stable_factories() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(rustok_product::ProductRuntimeSelected);
        extensions.insert(rustok_index::IndexSchemaSourceCatalog::new());
        extensions.insert(rustok_index::PostgresIndexSourceFactoryCatalog::new());

        register(&mut extensions).unwrap();

        assert_eq!(
            extensions
                .get::<rustok_index::IndexSchemaSourceCatalog>()
                .unwrap()
                .len(),
            4
        );
        let factories = extensions
            .get::<rustok_index::PostgresIndexSourceFactoryCatalog>()
            .unwrap();
        assert_eq!(factories.len(), 2);
        assert!(factories.iter().any(|factory| {
            factory.owner_module() == "product"
                && factory.factory_name() == PRODUCT_INDEX_SOURCE
        }));
        assert!(factories.iter().any(|factory| {
            factory.owner_module() == "product"
                && factory.factory_name() == PRODUCT_VARIANT_INDEX_SOURCE
        }));
    }
}
