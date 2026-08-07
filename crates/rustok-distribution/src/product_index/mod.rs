mod absence;
pub(crate) mod channel_relation_resolver;
#[cfg(test)]
pub(crate) use absence::PRODUCT_ABSENCE_WATERMARK_FACTORY;
mod product;
#[cfg(test)]
pub(crate) use product::PRODUCT_INDEX_SOURCE;
pub(crate) mod relation_admission;
#[path = "../product_variant_index.rs"]
mod variant;
#[cfg(test)]
pub(crate) use variant::PRODUCT_VARIANT_INDEX_SOURCE;

pub(crate) fn register(
    extensions: &mut rustok_core::ModuleRuntimeExtensions,
) -> rustok_core::Result<()> {
    product::register(extensions)?;
    variant::register(extensions)?;
    absence::register(extensions)
}

#[cfg(test)]
mod tests {
    use rustok_core::ModuleRuntimeExtensions;

    use super::{
        PRODUCT_ABSENCE_WATERMARK_FACTORY, PRODUCT_INDEX_SOURCE, PRODUCT_VARIANT_INDEX_SOURCE,
        register,
    };

    #[test]
    fn selected_product_bridge_registers_two_current_schemas_and_three_factories() {
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
            2
        );
        let factories = extensions
            .get::<rustok_index::PostgresIndexSourceFactoryCatalog>()
            .unwrap();
        assert_eq!(factories.len(), 3);
        assert!(factories.iter().any(|factory| {
            factory.owner_module() == "product"
                && factory.factory_name() == PRODUCT_INDEX_SOURCE
        }));
        assert!(factories.iter().any(|factory| {
            factory.owner_module() == "product"
                && factory.factory_name() == PRODUCT_VARIANT_INDEX_SOURCE
        }));
        assert!(factories.iter().any(|factory| {
            factory.owner_module() == "product"
                && factory.factory_name() == PRODUCT_ABSENCE_WATERMARK_FACTORY
        }));
    }
}
