mod manifest;

pub(crate) use manifest::{map_module_settings_validation_error, module_setting_shape_value};
pub use manifest::{
    validate_registry_vs_manifest, CatalogManifestModule, CatalogModuleVersion,
    DeploymentSurfaceContract, InstalledManifestModule, ManifestDiff, ManifestError,
    ManifestManager, ManifestModuleSpec, ModuleSettingSpec, ModulesManifest,
};
pub use rustok_distribution::build_registry;

#[cfg(test)]
mod contract_tests {
    use super::{build_registry, ManifestManager};
    use rustok_api::Permission;
    use rustok_api::{Action, Resource};
    use rustok_auth::AUTH_USER_PERMISSIONS;

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
    const SEO_README: &str = include_str!("../../../../crates/rustok-seo/README.md");
    const TAXONOMY_README: &str = include_str!("../../../../crates/rustok-taxonomy/README.md");
    const WORKFLOW_README: &str = include_str!("../../../../crates/rustok-workflow/README.md");
    const ALLOY_README: &str = include_str!("../../../../crates/alloy/README.md");
    const FLEX_README: &str = include_str!("../../../../crates/flex/README.md");
    const FLEX_MUTATION: &str = include_str!("../../../../crates/flex/src/graphql/mutation.rs");

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
            ("seo", SEO_README),
            ("taxonomy", TAXONOMY_README),
            ("workflow", WORKFLOW_README),
            ("alloy", ALLOY_README),
            ("flex", FLEX_README),
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
        let seo = registry.get("seo").expect("seo module");
        let taxonomy = registry.get("taxonomy").expect("taxonomy module");
        let workflow = registry.get("workflow").expect("workflow module");
        let alloy = registry.get("alloy").expect("alloy module");
        let flex = registry.get("flex").expect("flex module");

        assert_eq!(auth.permissions(), AUTH_USER_PERMISSIONS.to_vec());
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
        assert!(product.permissions().contains(&Permission::PRODUCTS_MANAGE));
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
        assert!(seo.permissions().contains(&Permission::SEO_MANAGE));
        assert!(taxonomy
            .permissions()
            .contains(&Permission::TAXONOMY_MANAGE));
        assert!(workflow
            .permissions()
            .contains(&Permission::WORKFLOWS_MANAGE));
        assert!(alloy.permissions().contains(&Permission::SCRIPTS_MANAGE));
        assert!(flex
            .permissions()
            .contains(&Permission::FLEX_SCHEMAS_MANAGE));
        assert!(flex
            .permissions()
            .contains(&Permission::FLEX_ENTRIES_MANAGE));
    }

    #[test]
    fn registry_dependencies_match_runtime_contract() {
        let registry = build_registry();
        let cart = registry.get("cart").expect("cart module");
        let channel = registry.get("channel").expect("channel module");
        let customer = registry.get("customer").expect("customer module");
        let product = registry.get("product").expect("product module");
        let profiles = registry.get("profiles").expect("profiles module");
        let region = registry.get("region").expect("region module");
        let payment = registry.get("payment").expect("payment module");
        let fulfillment = registry.get("fulfillment").expect("fulfillment module");
        let commerce = registry.get("commerce").expect("commerce module");
        let outbox = registry.get("outbox").expect("outbox module");
        let forum = registry.get("forum").expect("forum module");
        let pages = registry.get("pages").expect("pages module");
        let seo = registry.get("seo").expect("seo module");
        let taxonomy = registry.get("taxonomy").expect("taxonomy module");
        let workflow = registry.get("workflow").expect("workflow module");
        let alloy = registry.get("alloy").expect("alloy module");
        let flex = registry.get("flex").expect("flex module");

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
        assert_eq!(pages.dependencies(), &["content", "page_builder"]);
        assert_eq!(seo.dependencies(), &["content"]);
        assert_eq!(taxonomy.dependencies(), &["content"]);
        assert!(workflow.dependencies().is_empty());
        assert!(alloy.dependencies().is_empty());
        assert!(flex.dependencies().is_empty());
    }

    #[test]
    fn runtime_registry_dependencies_match_modules_manifest() {
        let manifest = ManifestManager::load().expect("modules.toml must load");
        let registry = build_registry();

        ManifestManager::validate_with_registry(&manifest, &registry)
            .expect("runtime registry dependencies must match modules.toml");
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
