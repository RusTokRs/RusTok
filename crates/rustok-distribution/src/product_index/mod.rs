mod absence;
mod channel_relation_convergence;
pub(crate) mod channel_relation_resolver;
mod channel_visibility;
#[cfg(test)]
pub(crate) use absence::PRODUCT_ABSENCE_WATERMARK_FACTORY;
mod product;
#[cfg(test)]
pub(crate) use product::PRODUCT_INDEX_SOURCE;
mod query_admission;
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
    absence::register(extensions)?;
    query_admission::register(extensions)?;
    channel_relation_convergence::register(extensions)
}

#[cfg(test)]
mod tests {
    use rustok_core::ModuleRuntimeExtensions;
    use rustok_runtime::ModuleWorkRegistrations;

    use super::{
        PRODUCT_ABSENCE_WATERMARK_FACTORY, PRODUCT_INDEX_SOURCE, PRODUCT_VARIANT_INDEX_SOURCE,
        register,
    };

    #[test]
    fn selected_product_bridge_registers_two_current_schemas_three_factories_and_query_admission() {
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
        let admissions = extensions
            .get::<rustok_index::PostgresIndexQueryAdmissionCatalog>()
            .expect("Product selection must publish one query admission rule");
        assert_eq!(admissions.len(), 1);
        assert!(!extensions.contains::<ModuleWorkRegistrations>());
    }

    #[test]
    fn selected_product_and_channel_bridge_registers_convergence_work() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(rustok_product::ProductRuntimeSelected);
        extensions.insert(rustok_channel::ChannelRuntimeSelected);
        extensions.insert(rustok_index::IndexSchemaSourceCatalog::new());
        extensions.insert(rustok_index::PostgresIndexSourceFactoryCatalog::new());

        register(&mut extensions).unwrap();

        let registrations = extensions
            .get::<ModuleWorkRegistrations>()
            .expect("Product+Channel composition must publish convergence work");
        assert!(!registrations.is_empty());
    }
}
