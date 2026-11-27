use async_trait::async_trait;
use rustok_core::{
    MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry, RusToKModule,
};
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
#[cfg(feature = "marketplace-financial")]
use rustok_outbox::TransactionalEventBus;
use sea_orm_migration::MigrationTrait;

#[cfg(feature = "marketplace-financial")]
use crate::services::MarketplaceFinancialRuntime;
use crate::{migrations, services};

/// Ecommerce umbrella/root module for the commerce family and orchestration surface.
pub struct CommerceModule;

#[async_trait]
impl RusToKModule for CommerceModule {
    fn slug(&self) -> &'static str {
        "commerce"
    }

    fn name(&self) -> &'static str {
        "Ecommerce"
    }

    fn description(&self) -> &'static str {
        "Ecommerce umbrella/root module for the commerce family and orchestration surface"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        crate::RUNTIME_DEPENDENCIES
    }

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        ctx: &ModuleEventListenerContext<'_>,
    ) {
        if let Some(fulfillment_registry) =
            ctx.extensions.get::<FulfillmentProviderRegistry>().cloned()
        {
            registry.register(services::PaidOrderCreateLabelHandler::new(
                ctx.db.clone(),
                fulfillment_registry,
            ));
        }

        #[cfg(feature = "marketplace-financial")]
        {
            let Some(financial_runtime) =
                ctx.extensions.get::<MarketplaceFinancialRuntime>().cloned()
            else {
                tracing::warn!(
                    "commerce marketplace-financial capability requires MarketplaceFinancialRuntime in ModuleRuntimeExtensions"
                );
                return;
            };
            let Some(event_bus) = ctx.extensions.get::<TransactionalEventBus>().cloned() else {
                tracing::warn!(
                    "commerce marketplace-financial capability requires TransactionalEventBus in ModuleRuntimeExtensions"
                );
                return;
            };
            registry.register(services::MarketplacePaidOrderFinancialHandler::new(
                ctx.db.clone(),
                event_bus,
                financial_runtime.ledger_port(),
            ));
        }
    }
}

impl MigrationSource for CommerceModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commerce_module_metadata() {
        let module = CommerceModule;
        assert_eq!(module.slug(), "commerce");
        assert_eq!(module.name(), "Ecommerce");
        assert_eq!(
            module.dependencies(),
            &[
                "tenant",
                "cart",
                "customer",
                "product",
                "region",
                "pricing",
                "inventory",
                "order",
                "payment",
                "fulfillment",
            ]
        );
    }
}
