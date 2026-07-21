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
use serde::Serialize;

/// Immutable identity of the modules compiled into this distribution.
///
/// `revision` is a readable package release label; `hash` is the canonical
/// identity used by installer receipts and topology descriptors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompositionIdentity {
    pub revision: String,
    pub hash: String,
    pub modules: Vec<CompositionModule>,
}

/// Canonical module metadata included in a distribution composition hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompositionModule {
    pub slug: String,
    pub version: String,
    pub kind: CompositionModuleKind,
    pub dependencies: Vec<String>,
}

/// Stable module classification retained in the composition identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionModuleKind {
    Core,
    Optional,
}

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
    #[cfg(feature = "mod-marketplace_seller")]
    {
        registry = registry.register(rustok_marketplace_seller::MarketplaceSellerModule);
    }
    #[cfg(feature = "mod-marketplace_listing")]
    {
        registry = registry.register(rustok_marketplace_listing::MarketplaceListingModule);
    }
    #[cfg(feature = "mod-marketplace_allocation")]
    {
        registry = registry.register(rustok_marketplace_allocation::MarketplaceAllocationModule);
    }
    #[cfg(feature = "mod-marketplace_commission")]
    {
        registry = registry.register(rustok_marketplace_commission::MarketplaceCommissionModule);
    }
    #[cfg(feature = "mod-marketplace_ledger")]
    {
        registry = registry.register(rustok_marketplace_ledger::MarketplaceLedgerModule);
    }
    #[cfg(feature = "mod-marketplace_payout")]
    {
        registry = registry.register(rustok_marketplace_payout::MarketplacePayoutModule);
    }
    #[cfg(feature = "mod-marketplace")]
    {
        registry = registry.register(rustok_marketplace::MarketplaceModule);
    }
    #[cfg(feature = "mod-moderation")]
    {
        registry = registry.register(rustok_moderation::ModerationModule);
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

/// Returns the deterministic identity of the selected compile-time module set.
pub fn composition_identity() -> CompositionIdentity {
    let modules = build_registry()
        .list()
        .into_iter()
        .map(|module| {
            let mut dependencies = module
                .dependencies()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            dependencies.sort();
            CompositionModule {
                slug: module.slug().to_string(),
                version: module.version().to_string(),
                kind: match module.kind() {
                    rustok_core::ModuleKind::Core => CompositionModuleKind::Core,
                    rustok_core::ModuleKind::Optional => CompositionModuleKind::Optional,
                },
                dependencies,
            }
        })
        .collect::<Vec<_>>();
    let revision = format!("rustok-distribution@{}", env!("CARGO_PKG_VERSION"));
    let snapshot = serde_json::json!({
        "schema_version": 1,
        "revision": &revision,
        "modules": &modules,
    });
    let hash = rustok_api::manifest_hash::hash_manifest_snapshot(&snapshot);

    CompositionIdentity {
        revision,
        hash,
        modules,
    }
}

#[cfg(test)]
mod tests {
    use super::composition_identity;

    #[test]
    fn selected_composition_identity_is_stable_and_contains_modules() {
        let first = composition_identity();
        let second = composition_identity();

        assert_eq!(first, second);
        assert_eq!(first.hash.len(), 64);
        assert!(first
            .hash
            .chars()
            .all(|character| character.is_ascii_hexdigit()));
        assert!(first.modules.iter().any(|module| module.slug == "tenant"));
    }
}