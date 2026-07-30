//! Selected distribution module composition shared by executable hosts.
//!
//! The crate owns only compile-time selection and `ModuleRegistry` assembly.
//! HTTP routing remains in `apps/server`; command providers remain in their
//! module-local CLI adapters.

mod generated_promotions;
mod generation;

use rustok_auth::AuthModule;
use rustok_cache::CacheModule;
use rustok_channel::ChannelModule;
use rustok_core::{ModuleRegistry, ModuleRuntimeExtensions};
use rustok_email::EmailModule;
use rustok_events_module::EventsModule;
use rustok_index::IndexModule;
use rustok_modules::ModulesModule;
use rustok_outbox::OutboxModule;
use rustok_rbac::RbacModule;
use rustok_search::SearchModule;
use rustok_social_graph::SocialGraphModule;
use rustok_tenant::TenantModule;
use serde::Serialize;

pub use generation::{
    GENERATED_DISTRIBUTION_CARGO_MANIFEST_PATH, GENERATED_DISTRIBUTION_MANIFEST_PATH,
    GENERATED_DISTRIBUTION_REGISTRY_PATH, GeneratedStaticDistributionFiles,
    GeneratedStaticDistributionManifest, GeneratedStaticDistributionSource,
    StaticDistributionGenerationError, generate_static_distribution,
};

/// Builds module-owned runtime extensions and then adds explicitly selected
/// cross-module adapters at the distribution composition boundary.
///
/// Executable hosts call this single neutral entrypoint and never import
/// adapter or owner capability types.
pub fn build_runtime_extensions(
    registry: &ModuleRegistry,
) -> rustok_core::Result<ModuleRuntimeExtensions> {
    let mut extensions = registry.build_runtime_extensions()?;
    register_runtime_bridges(&mut extensions)?;
    Ok(extensions)
}

fn register_runtime_bridges(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    #[cfg(feature = "ai-translation")]
    {
        if extensions.contains::<rustok_translation::SharedMachineTranslationPortFactory>() {
            return Err(rustok_core::Error::Validation(
                "machine translation runtime factory is already registered".to_string(),
            ));
        }
        extensions.insert(rustok_translation::SharedMachineTranslationPortFactory(
            std::sync::Arc::new(rustok_ai_translation::AiMachineTranslationPortFactory),
        ));
    }
    #[cfg(not(feature = "ai-translation"))]
    let _ = extensions;
    Ok(())
}

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
        .register(EventsModule)
        .register(TenantModule)
        .register(RbacModule)
        .register(SocialGraphModule);

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
    #[cfg(feature = "mod-notifications")]
    {
        registry = registry.register(rustok_notifications::NotificationsModule);
    }
    #[cfg(feature = "mod-comments")]
    {
        registry = registry.register(rustok_comments::CommentsModule);
    }
    #[cfg(feature = "mod-pages")]
    {
        registry = registry.register(rustok_pages::PagesModule);
    }
    #[cfg(feature = "mod-navigation")]
    {
        registry = registry.register(rustok_navigation::NavigationModule);
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
    #[cfg(feature = "mod-translation")]
    {
        registry = registry.register(rustok_translation::TranslationModule);
    }
    #[cfg(feature = "mod-seo")]
    {
        registry = registry.register(rustok_seo::SeoModule);
    }
    #[cfg(feature = "mod-workflow")]
    {
        registry = registry.register(rustok_workflow::WorkflowModule);
    }
    #[cfg(feature = "mod-ai")]
    {
        registry = registry.register(rustok_ai::AiModule);
    }

    generated_promotions::register_promoted_modules(registry)
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
        assert!(
            first
                .hash
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        );
        assert!(first.modules.iter().any(|module| module.slug == "tenant"));
        assert!(
            first
                .modules
                .iter()
                .any(|module| module.slug == "social_graph")
        );
        #[cfg(feature = "mod-ai")]
        assert!(first.modules.iter().any(|module| module.slug == "ai"));
    }

    #[cfg(feature = "ai-translation")]
    #[tokio::test]
    async fn selected_ai_translation_bridge_publishes_factory_and_stays_optional_without_keyring() {
        let registry = super::build_registry();
        let extensions =
            super::build_runtime_extensions(&registry).expect("distribution runtime extensions");
        let factory = extensions
            .get::<rustok_translation::SharedMachineTranslationPortFactory>()
            .cloned()
            .expect("selected AI Translation bridge must publish its owner-neutral factory");
        let database = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("isolated runtime database");
        let context = rustok_api::HostRuntimeContext::new(database).with_shared_value(factory);

        assert!(
            rustok_translation::machine_translation_port_from_context(&context)
                .expect("missing deployment keyring is an optional provider state")
                .is_none(),
            "manual Translation workflows must remain available when AI keyring provisioning is absent"
        );
    }
}
