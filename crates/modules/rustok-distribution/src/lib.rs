//! # RusToK Distribution
//!
//! Selected distribution module composition shared by executable hosts.
//! The crate owns only compile-time selection and `ModuleRegistry` assembly.
//! HTTP routing remains in `apps/server`; command providers remain in their
//! module-local CLI adapters.

mod channel_index;
mod generated_promotions;
mod generation;
#[cfg(feature = "mod-product")]
pub mod product_index;

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

fn register_selected_index_bridges(
    extensions: &mut ModuleRuntimeExtensions,
) -> rustok_core::Result<()> {
    channel_index::register(extensions)?;
    #[cfg(feature = "mod-product")]
    product_index::register(extensions)?;
    Ok(())
}

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
    register_selected_index_bridges(&mut extensions)?;
    materialize_index_schema_sources(&mut extensions)?;
    Ok(extensions)
}

fn materialize_index_schema_sources(
    extensions: &mut ModuleRuntimeExtensions,
) -> rustok_core::Result<()> {
    if extensions.contains::<rustok_index::SharedIndexSchemaRegistry>() {
        return Err(rustok_core::Error::Validation(
            "shared Index schema registry is already materialized".to_string(),
        ));
    }

    let shared = rustok_index::materialize_index_schema_registry(extensions).map_err(|error| {
        rustok_core::Error::Validation(format!(
            "Index source schema registry materialization failed: {error}"
        ))
    })?;
    if let Some(shared) = shared {
        extensions.insert(shared);
    }
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
    let registry = ModuleRegistry::new()
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
    let registry = registry.register(rustok_cart::CartModule);
    #[cfg(feature = "mod-customer")]
    let registry = registry.register(rustok_customer::CustomerModule);
    #[cfg(feature = "mod-product")]
    let registry = registry.register(rustok_product::ProductModule);
    #[cfg(feature = "mod-product_relations")]
    let registry = registry.register(rustok_product_relations::ProductRelationsModule);
    #[cfg(feature = "mod-brand")]
    let registry = registry.register(rustok_brand::BrandModule);
    #[cfg(feature = "mod-product_bundles")]
    let registry = registry.register(rustok_product_bundles::ProductBundlesModule);
    #[cfg(feature = "mod-profiles")]
    let registry = registry.register(rustok_profiles::ProfilesModule);
    #[cfg(feature = "mod-groups")]
    let registry = registry.register(rustok_groups::GroupsModule);
    #[cfg(feature = "mod-region")]
    let registry = registry.register(rustok_region::RegionModule);
    #[cfg(feature = "mod-pricing")]
    let registry = registry.register(rustok_pricing::PricingModule);
    #[cfg(feature = "mod-inventory")]
    let registry = registry.register(rustok_inventory::InventoryModule);
    #[cfg(feature = "mod-order")]
    let registry = registry.register(rustok_order::OrderModule);
    #[cfg(feature = "mod-payment")]
    let registry = registry.register(rustok_payment::PaymentModule);
    #[cfg(feature = "mod-fulfillment")]
    let registry = registry.register(rustok_fulfillment::FulfillmentModule);
    #[cfg(feature = "mod-commerce")]
    let registry = registry.register(rustok_commerce::CommerceModule);
    #[cfg(feature = "mod-marketplace_seller")]
    let registry = registry.register(rustok_marketplace_seller::MarketplaceSellerModule);
    #[cfg(feature = "mod-marketplace_listing")]
    let registry = registry.register(rustok_marketplace_listing::MarketplaceListingModule);
    #[cfg(feature = "mod-marketplace_allocation")]
    let registry = registry.register(rustok_marketplace_allocation::MarketplaceAllocationModule);
    #[cfg(feature = "mod-marketplace_commission")]
    let registry = registry.register(rustok_marketplace_commission::MarketplaceCommissionModule);
    #[cfg(feature = "mod-marketplace_balance")]
    let registry = registry.register(rustok_marketplace_balance::MarketplaceBalanceModule);
    #[cfg(feature = "mod-marketplace_payout")]
    let registry = registry.register(rustok_marketplace_payout::MarketplacePayoutModule);
    #[cfg(feature = "mod-marketplace")]
    let registry = registry.register(rustok_marketplace::MarketplaceModule);
    #[cfg(feature = "mod-accounting")]
    let registry = registry.register(rustok_accounting::AccountingModule);
    #[cfg(feature = "mod-moderation")]
    let registry = registry.register(rustok_moderation::ModerationModule);
    #[cfg(feature = "mod-content")]
    let registry = registry.register(rustok_content::ContentModule);
    #[cfg(feature = "mod-blog")]
    let registry = registry.register(rustok_blog::BlogModule);
    #[cfg(feature = "mod-reactions")]
    let registry = registry.register(rustok_reactions::ReactionsModule);
    #[cfg(feature = "mod-forum")]
    let registry = registry.register(rustok_forum::ForumModule);
    #[cfg(feature = "mod-notifications")]
    let registry = registry.register(rustok_notifications::NotificationsModule);
    #[cfg(feature = "mod-newsletter")]
    let registry = registry.register(rustok_newsletter::NewsletterModule);
    #[cfg(feature = "mod-comments")]
    let registry = registry.register(rustok_comments::CommentsModule);
    #[cfg(feature = "mod-pages")]
    let registry = registry.register(rustok_pages::PagesModule);
    #[cfg(feature = "mod-forms")]
    let registry = registry.register(rustok_forms::FormsModule);
    #[cfg(feature = "mod-navigation")]
    let registry = registry.register(rustok_navigation::NavigationModule);
    #[cfg(feature = "mod-page_builder")]
    let registry = registry.register(rustok_page_builder::PageBuilderModule);
    #[cfg(feature = "mod-taxonomy")]
    let registry = registry.register(rustok_taxonomy::TaxonomyModule);
    #[cfg(feature = "mod-alloy")]
    let registry = registry.register(alloy::AlloyModule);
    #[cfg(feature = "mod-flex")]
    let registry = registry.register(flex::FlexModule);
    #[cfg(feature = "mod-media")]
    let registry = registry.register(rustok_media::MediaModule);
    #[cfg(feature = "mod-translation")]
    let registry = registry.register(rustok_translation::TranslationModule);
    #[cfg(feature = "mod-seo")]
    let registry = registry.register(rustok_seo::SeoModule);
    #[cfg(feature = "mod-workflow")]
    let registry = registry.register(rustok_workflow::WorkflowModule);
    #[cfg(feature = "mod-security")]
    let registry = registry.register(rustok_security::SecurityModule);
    #[cfg(feature = "mod-ai")]
    let registry = registry.register(rustok_ai::AiModule);
    #[cfg(feature = "mod-docs")]
    let registry = registry.register(rustok_docs::DocsModule);
    #[cfg(feature = "mod-messenger")]
    let registry = registry.register(rustok_messenger::MessengerModule);

    #[cfg(feature = "mod-party")]
    let registry = registry.register(rustok_party::PartyModule);
    #[cfg(feature = "mod-deal")]
    let registry = registry.register(rustok_deal::DealModule);
    #[cfg(feature = "mod-billing")]
    let registry = registry.register(rustok_billing::BillingModule);
    #[cfg(feature = "mod-finance")]
    let registry = registry.register(rustok_finance::FinanceModule);
    #[cfg(feature = "mod-calls")]
    let registry = registry.register(rustok_calls::CallsModule);
    #[cfg(feature = "mod-calendar")]
    let registry = registry.register(rustok_calendar::CalendarModule);
    #[cfg(feature = "mod-tasks")]
    let registry = registry.register(rustok_tasks::TasksModule);
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
mod tests;

#[cfg(test)]
mod contract_tests;
