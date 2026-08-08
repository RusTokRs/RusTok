mod absence;
mod attribute_terms;
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
mod storefront_budgeted_execution;
pub(crate) use storefront_budgeted_execution::{
    ProductStorefrontIndexBudgetedExecution, ProductStorefrontIndexBudgetedProjectionError,
    ProductStorefrontIndexBudgetedProjectionExecutor, ProductStorefrontIndexBudgetedStartError,
    ProductStorefrontIndexBudgetedTagHydrationError,
};
mod storefront_projection;
pub(crate) use storefront_projection::{
    ProductStorefrontIndexPublicProjectionError, project_product_storefront_index_page,
};
mod storefront_serving_budget;
pub(crate) use storefront_serving_budget::{
    ProductStorefrontIndexServingBudget, ProductStorefrontIndexServingBudgetDecision,
    ProductStorefrontIndexServingBudgetError, ProductStorefrontIndexServingBudgetObservation,
    classify_product_storefront_index_serving_budget,
};
mod storefront_shadow;
pub(crate) use storefront_shadow::{
    ProductStorefrontIndexShadowError, build_product_storefront_index_shadow_query,
};
mod storefront_shadow_executor;
pub(crate) use storefront_shadow_executor::{
    ProductStorefrontIndexChannelScopeDecision, ProductStorefrontIndexPageScopeDecision,
    ProductStorefrontIndexShadowComparison, ProductStorefrontIndexShadowExecution,
    ProductStorefrontIndexShadowExecutor, ProductStorefrontIndexShadowProjectionError,
    ProductStorefrontIndexTagHydrationError, classify_product_storefront_index_channel_scope,
    classify_product_storefront_index_page_scope,
};
#[cfg(test)]
mod storefront_shadow_eav_postgres_tests;
#[cfg(test)]
mod storefront_shadow_postgres_tests;
#[path = "../product_variant_index.rs"]
mod variant;
#[cfg(test)]
pub(crate) use variant::PRODUCT_VARIANT_INDEX_SOURCE;

/// Internal persisted routing key for the one Product schema published by current runtime code.
///
/// Lower keys are historical storage identities only. They are never selected as compatibility
/// implementations by this module.
pub(crate) const PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4;

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
    fn selected_product_bridge_registers_two_current_schemas_three_factories_and_entity_admissions() {
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
            .expect("Product selection must publish Product and ProductVariant query admissions");
        assert_eq!(admissions.len(), 2);
        assert_eq!(admissions.link_availability_len(), 1);
        assert!(!extensions.contains::<ModuleWorkRegistrations>());
    }

    #[test]
    fn selected_product_and_channel_bridge_registers_channel_admission_and_convergence_work() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(rustok_product::ProductRuntimeSelected);
        extensions.insert(rustok_channel::ChannelRuntimeSelected);
        extensions.insert(rustok_index::IndexSchemaSourceCatalog::new());
        extensions.insert(rustok_index::PostgresIndexSourceFactoryCatalog::new());

        register(&mut extensions).unwrap();

        let admissions = extensions
            .get::<rustok_index::PostgresIndexQueryAdmissionCatalog>()
            .expect("Product+Channel selection must publish graph entity admissions");
        assert_eq!(admissions.len(), 3);
        assert_eq!(admissions.link_availability_len(), 1);
        let registrations = extensions
            .get::<ModuleWorkRegistrations>()
            .expect("Product+Channel composition must publish convergence work");
        assert!(!registrations.is_empty());
    }
}
