mod manifest;
mod registry_codegen {
    include!(concat!(env!("OUT_DIR"), "/modules_registry_codegen.rs"));
}

use rustok_auth::AuthModule;
use rustok_cache::CacheModule;
use rustok_channel::ChannelModule;
use rustok_core::ModuleRegistry;
use rustok_email::EmailModule;
use rustok_index::IndexModule;
use rustok_outbox::OutboxModule;
use rustok_rbac::RbacModule;
use rustok_search::SearchModule;
use rustok_tenant::TenantModule;

pub use manifest::{
    validate_registry_vs_manifest, BuildExecutionPlan, CatalogManifestModule, CatalogModuleVersion,
    DeploymentSurfaceContract, FrontendArtifactKind, FrontendBuildPlan, FrontendBuildTool,
    InstalledManifestModule, ManifestDiff, ManifestError, ManifestManager, ManifestModuleSpec,
    ModuleSettingSpec, ModulesManifest,
};

pub fn build_registry() -> ModuleRegistry {
    let cache_module = CacheModule::new();
    let registry = ModuleRegistry::new()
        .register(AuthModule)
        .register(cache_module)
        .register(ChannelModule)
        .register(EmailModule)
        .register(IndexModule)
        .register(SearchModule)
        .register(OutboxModule)
        .register(TenantModule)
        .register(RbacModule);

    registry_codegen::register_optional_modules(registry)
}

#[cfg(test)]
mod contract_tests {
    use super::build_registry;
    use rustok_core::permissions::{Action, Resource};
    use rustok_core::Permission;

    const AUTH_README: &str = include_str!("../../../../crates/rustok-auth/README.md");
    const CACHE_README: &str = include_str!("../../../../crates/rustok-cache/README.md");
    const CHANNEL_README: &str = include_str!("../../../../crates/rustok-channel/README.md");
    const EMAIL_README: &str = include_str!("../../../../crates/rustok-email/README.md");
    const INDEX_README: &str = include_str!("../../../../crates/rustok-index/README.md");
    const SEARCH_README: &str = include_str!("../../../../crates/rustok-search/README.md");
    const OUTBOX_README: &str = include_str!("../../../../crates/rustok-outbox/README.md");
    const TENANT_README: &str = include_str!("../../../../crates/rustok-tenant/README.md");
    const RBAC_README: &str = include_str!("../../../../crates/rustok-rbac/README.md");
    const CONTENT_README: &str = include_str!("../../../../crates/rustok-content/README.md");
    const CART_README: &str = include_str!("../../../../crates/rustok-cart/README.md");
    const CUSTOMER_README: &str = include_str!("../../../../crates/rustok-customer/README.md");
    const PROFILES_README: &str = include_str!("../../../../crates/rustok-profiles/README.md");
    const REGION_README: &str = include_str!("../../../../crates/rustok-region/README.md");
    const ORDER_README: &str = include_str!("../../../../crates/rustok-order/README.md");
    const PAYMENT_README: &str = include_str!("../../../../crates/rustok-payment/README.md");
    const FULFILLMENT_README: &str =
        include_str!("../../../../crates/rustok-fulfillment/README.md");
    const COMMERCE_README: &str = include_str!("../../../../crates/rustok-commerce/README.md");
    const BLOG_README: &str = include_str!("../../../../crates/rustok-blog/README.md");
    const FORUM_README: &str = include_str!("../../../../crates/rustok-forum/README.md");
    const MEDIA_README: &str = include_str!("../../../../crates/rustok-media/README.md");
    const PAGES_README: &str = include_str!("../../../../crates/rustok-pages/README.md");
    const TAXONOMY_README: &str = include_str!("../../../../crates/rustok-taxonomy/README.md");
    const WORKFLOW_README: &str = include_str!("../../../../crates/rustok-workflow/README.md");
    const FLEX_MUTATION: &str = include_str!("../graphql/flex/mutation.rs");

    #[test]
    fn registry_module_readmes_define_interactions_section() {
        for (slug, readme) in [
            ("auth", AUTH_README),
            ("cache", CACHE_README),
            ("channel", CHANNEL_README),
            ("email", EMAIL_README),
            ("index", INDEX_README),
            ("search", SEARCH_README),
            ("outbox", OUTBOX_README),
            ("tenant", TENANT_README),
            ("rbac", RBAC_README),
            ("content", CONTENT_README),
            ("cart", CART_README),
            ("customer", CUSTOMER_README),
            ("profiles", PROFILES_README),
            ("region", REGION_README),
            ("order", ORDER_README),
            ("payment", PAYMENT_README),
            ("fulfillment", FULFILLMENT_README),
            ("commerce", COMMERCE_README),
            ("blog", BLOG_README),
            ("forum", FORUM_README),
            ("media", MEDIA_README),
            ("pages", PAGES_README),
            ("taxonomy", TAXONOMY_README),
            ("workflow", WORKFLOW_README),
        ] {
            assert!(
                readme.contains("## Interactions"),
                "module `{slug}` README must contain `## Interactions`"
            );
        }
    }

    #[test]
    fn registry_modules_publish_expected_rbac_surface() {
        let registry = build_registry();
        let auth = registry.get("auth").expect("auth module");
        let channel = registry.get("channel").expect("channel module");
        let tenant = registry.get("tenant").expect("tenant module");
        let rbac = registry.get("rbac").expect("rbac module");
        let blog = registry.get("blog").expect("blog module");
        let forum = registry.get("forum").expect("forum module");
        let media = registry.get("media").expect("media module");
        let cart = registry.get("cart").expect("cart module");
        let customer = registry.get("customer").expect("customer module");
        let product = registry.get("product").expect("product module");
        let profiles = registry.get("profiles").expect("profiles module");
        let order = registry.get("order").expect("order module");
        let payment = registry.get("payment").expect("payment module");
        let region = registry.get("region").expect("region module");
        let fulfillment = registry.get("fulfillment").expect("fulfillment module");
        let pages = registry.get("pages").expect("pages module");
        let taxonomy = registry.get("taxonomy").expect("taxonomy module");
        let workflow = registry.get("workflow").expect("workflow module");

        assert!(auth.permissions().contains(&Permission::USERS_MANAGE));
        assert!(channel.permissions().is_empty());
        assert!(tenant.permissions().contains(&Permission::TENANTS_MANAGE));
        assert!(tenant.permissions().contains(&Permission::MODULES_MANAGE));
        assert!(rbac.permissions().contains(&Permission::SETTINGS_MANAGE));
        assert!(rbac.permissions().contains(&Permission::LOGS_READ));
        assert!(blog.permissions().contains(&Permission::BLOG_POSTS_MANAGE));
        assert!(forum
            .permissions()
            .contains(&Permission::FORUM_TOPICS_MANAGE));
        assert!(cart.permissions().is_empty());
        assert!(customer
            .permissions()
            .contains(&Permission::CUSTOMERS_MANAGE));
        assert!(profiles
            .permissions()
            .contains(&Permission::PROFILES_MANAGE));
        assert!(region.permissions().contains(&Permission::REGIONS_MANAGE));
        assert!(order.permissions().contains(&Permission::ORDERS_MANAGE));
        assert!(payment.permissions().contains(&Permission::PAYMENTS_MANAGE));
        assert!(fulfillment
            .permissions()
            .contains(&Permission::FULFILLMENTS_MANAGE));
        assert!(media
            .permissions()
            .contains(&Permission::new(Resource::Media, Action::Manage)));
        assert!(pages.permissions().contains(&Permission::PAGES_MANAGE));
        assert!(taxonomy
            .permissions()
            .contains(&Permission::TAXONOMY_MANAGE));
        assert!(workflow
            .permissions()
            .contains(&Permission::WORKFLOWS_MANAGE));
    }

    #[test]
    fn registry_dependencies_match_runtime_contract() {
        let registry = build_registry();
        let cart = registry.get("cart").expect("cart module");
        let channel = registry.get("channel").expect("channel module");
        let customer = registry.get("customer").expect("customer module");
        let profiles = registry.get("profiles").expect("profiles module");
        let region = registry.get("region").expect("region module");
        let payment = registry.get("payment").expect("payment module");
        let fulfillment = registry.get("fulfillment").expect("fulfillment module");
        let commerce = registry.get("commerce").expect("commerce module");
        let outbox = registry.get("outbox").expect("outbox module");
        let forum = registry.get("forum").expect("forum module");
        let pages = registry.get("pages").expect("pages module");
        let taxonomy = registry.get("taxonomy").expect("taxonomy module");
        let workflow = registry.get("workflow").expect("workflow module");

        assert!(registry.is_core("channel"));
        assert!(registry.is_core("outbox"));
        assert!(channel.dependencies().is_empty());
        assert!(cart.dependencies().is_empty());
        assert!(customer.dependencies().is_empty());
        assert_eq!(product.dependencies(), &["taxonomy"]);
        assert_eq!(profiles.dependencies(), &["taxonomy"]);
        assert!(region.dependencies().is_empty());
        assert!(payment.dependencies().is_empty());
        assert!(fulfillment.dependencies().is_empty());
        assert!(outbox.dependencies().is_empty());
        assert_eq!(
            commerce.dependencies(),
            &[
                "cart",
                "customer",
                "product",
                "region",
                "pricing",
                "inventory",
                "order",
                "payment",
                "fulfillment"
            ]
        );
        assert_eq!(forum.dependencies(), &["content", "taxonomy"]);
        assert_eq!(pages.dependencies(), &["content"]);
        assert_eq!(taxonomy.dependencies(), &["content"]);
        assert!(workflow.dependencies().is_empty());
    }

    #[test]
    fn flex_mutation_uses_explicit_permissions_only() {
        assert!(FLEX_MUTATION.contains("FLEX_SCHEMAS_"));
        assert!(
            !FLEX_MUTATION.contains("infer_user_role_from_permissions"),
            "Flex authorization must not use role inference"
        );
        assert!(
            !FLEX_MUTATION.contains("UserRole::Admin")
                && !FLEX_MUTATION.contains("UserRole::SuperAdmin"),
            "Flex authorization must not hardcode role checks"
        );
    }
}
