//! Selected distribution module composition shared by executable hosts.
//!
//! The crate owns only compile-time selection and `ModuleRegistry` assembly.
//! HTTP routing remains in `apps/server`; command providers remain in their
//! module-local CLI adapters.

use rustok_auth::AuthModule;
use rustok_cache::CacheModule;
use rustok_channel::ChannelModule;
use rustok_core::ModuleRegistry;
use rustok_email::EmailModule;
use rustok_index::IndexModule;
use rustok_modules::ModulesModule;
use rustok_outbox::OutboxModule;
use rustok_rbac::RbacModule;
use rustok_search::SearchModule;
use rustok_tenant::TenantModule;

/// Builds the module registry for the features selected in this distribution.
pub fn build_registry() -> ModuleRegistry {
    let mut registry = ModuleRegistry::new()
        .register(ModulesModule)
        .register(AuthModule)
        .register(CacheModule::new())
        .register(ChannelModule)
        .register(EmailModule)
        .register(IndexModule)
        .register(SearchModule)
        .register(OutboxModule)
        .register(TenantModule)
        .register(RbacModule);

    #[cfg(feature = "mod-cart")]
    {
        registry = registry.register(rustok_cart::CartModule);
    }
    #[cfg(feature = "mod-customer")]
    {
        registry = registry.register(rustok_customer::CustomerModule);
    }
    #[cfg(feature = "mod-product")]
    {
        registry = registry.register(rustok_product::ProductModule);
    }
    #[cfg(feature = "mod-profiles")]
    {
        registry = registry.register(rustok_profiles::ProfilesModule);
    }
    #[cfg(feature = "mod-region")]
    {
        registry = registry.register(rustok_region::RegionModule);
    }
    #[cfg(feature = "mod-pricing")]
    {
        registry = registry.register(rustok_pricing::PricingModule);
    }
    #[cfg(feature = "mod-inventory")]
    {
        registry = registry.register(rustok_inventory::InventoryModule);
    }
    #[cfg(feature = "mod-order")]
    {
        registry = registry.register(rustok_order::OrderModule);
    }
    #[cfg(feature = "mod-payment")]
    {
        registry = registry.register(rustok_payment::PaymentModule);
    }
    #[cfg(feature = "mod-fulfillment")]
    {
        registry = registry.register(rustok_fulfillment::FulfillmentModule);
    }
    #[cfg(feature = "mod-commerce")]
    {
        registry = registry.register(rustok_commerce::CommerceModule);
    }
    #[cfg(feature = "mod-content")]
    {
        registry = registry.register(rustok_content::ContentModule);
    }
    #[cfg(feature = "mod-blog")]
    {
        registry = registry.register(rustok_blog::BlogModule);
    }
    #[cfg(feature = "mod-forum")]
    {
        registry = registry.register(rustok_forum::ForumModule);
    }
    #[cfg(feature = "mod-comments")]
    {
        registry = registry.register(rustok_comments::CommentsModule);
    }
    #[cfg(feature = "mod-pages")]
    {
        registry = registry.register(rustok_pages::PagesModule);
    }
    #[cfg(feature = "mod-page_builder")]
    {
        registry = registry.register(rustok_page_builder::PageBuilderModule);
    }
    #[cfg(feature = "mod-taxonomy")]
    {
        registry = registry.register(rustok_taxonomy::TaxonomyModule);
    }
    #[cfg(feature = "mod-alloy")]
    {
        registry = registry.register(alloy::AlloyModule);
    }
    #[cfg(feature = "mod-flex")]
    {
        registry = registry.register(flex::FlexModule);
    }
    #[cfg(feature = "mod-media")]
    {
        registry = registry.register(rustok_media::MediaModule);
    }
    #[cfg(feature = "mod-seo")]
    {
        registry = registry.register(rustok_seo::SeoModule);
    }
    #[cfg(feature = "mod-workflow")]
    {
        registry = registry.register(rustok_workflow::WorkflowModule);
    }

    registry
}
