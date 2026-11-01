pub(crate) mod manifest;

pub(crate) use manifest::map_module_settings_validation_error;
pub use manifest::{
    CatalogManifestModule, CatalogModuleVersion, DeploymentSurfaceContract,
    InstalledManifestModule, ManifestDiff, ManifestError, ManifestManager, ManifestModuleSpec,
    ModuleSettingSpec, ModulesManifest, validate_registry_vs_manifest,
};
pub use rustok_distribution::build_registry;

#[cfg(test)]
mod contract_tests {
    use super::{ManifestManager, build_registry};
    use rustok_api::Permission;
    use rustok_api::{Action, Resource};
    use rustok_auth::AUTH_USER_PERMISSIONS;
    use serial_test::serial;

    const AUTH_README: &str = include_str!("../../../../crates/modules/rustok-auth/README.md");
    const CACHE_README: &str = include_str!("../../../../crates/modules/rustok-cache/README.md");
    const CHANNEL_README: &str =
        include_str!("../../../../crates/modules/rustok-channel/README.md");
    const EMAIL_README: &str = include_str!("../../../../crates/modules/rustok-email/README.md");
    const INDEX_README: &str = include_str!("../../../../crates/modules/rustok-index/README.md");
    const SEARCH_README: &str = include_str!("../../../../crates/modules/rustok-search/README.md");
    const OUTBOX_README: &str = include_str!("../../../../crates/modules/rustok-outbox/README.md");
    const TENANT_README: &str = include_str!("../../../../crates/modules/rustok-tenant/README.md");
    const RBAC_README: &str = include_str!("../../../../crates/modules/rustok-rbac/README.md");
    const CONTENT_README: &str =
        include_str!("../../../../crates/modules/rustok-content/README.md");
    const CART_README: &str = include_str!("../../../../crates/modules/rustok-cart/README.md");
    const CUSTOMER_README: &str =
        include_str!("../../../../crates/modules/rustok-customer/README.md");
    const PROFILES_README: &str =
        include_str!("../../../../crates/modules/rustok-profiles/README.md");
    const REGION_README: &str = include_str!("../../../../crates/modules/rustok-region/README.md");
    const ORDER_README: &str = include_str!("../../../../crates/modules/rustok-order/README.md");
    const PAYMENT_README: &str =
        include_str!("../../../../crates/modules/rustok-payment/README.md");
    const FULFILLMENT_README: &str =
        include_str!("../../../../crates/modules/rustok-fulfillment/README.md");
    const COMMERCE_README: &str =
        include_str!("../../../../crates/modules/rustok-commerce/README.md");
    const BLOG_README: &str = include_str!("../../../../crates/modules/rustok-blog/README.md");
    const FORUM_README: &str = include_str!("../../../../crates/modules/rustok-forum/README.md");
    const MEDIA_README: &str = include_str!("../../../../crates/modules/rustok-media/README.md");
    const PAGES_README: &str = include_str!("../../../../crates/modules/rustok-pages/README.md");
    const SEO_README: &str = include_str!("../../../../crates/modules/rustok-seo/README.md");
    const TAXONOMY_README: &str =
        include_str!("../../../../crates/modules/rustok-taxonomy/README.md");
    const WORKFLOW_README: &str =
        include_str!("../../../../crates/modules/rustok-workflow/README.md");
    const ALLOY_README: &str = include_str!("../../../../crates/modules/alloy/README.md");
    const FLEX_README: &str = include_str!("../../../../crates/modules/flex/README.md");
    const FLEX_MUTATION: &str =
        include_str!("../../../../crates/modules/flex/src/graphql/mutation.rs");

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
    #[test]
    fn registry_modules_publish_expected_rbac_surface() {
        let registry = build_registry();
        let auth = registry.get("auth").expect("auth module");
        let channel = registry.get("channel").expect("channel module");
        let tenant = registry.get("tenant").expect("tenant module");
        let rbac = registry.get("rbac").expect("rbac module");

        assert_eq!(auth.permissions(), AUTH_USER_PERMISSIONS.to_vec());
        assert!(channel.permissions().is_empty());
        assert!(tenant.permissions().contains(&Permission::TENANTS_MANAGE));
        assert!(tenant.permissions().contains(&Permission::MODULES_MANAGE));
        assert!(rbac.permissions().contains(&Permission::SETTINGS_MANAGE));
        assert!(rbac.permissions().contains(&Permission::LOGS_READ));

        if let Some(blog) = registry.get("blog") {
            assert!(blog.permissions().contains(&Permission::BLOG_POSTS_MANAGE));
        }
        if let Some(forum) = registry.get("forum") {
            assert!(
                forum
                    .permissions()
                    .contains(&Permission::FORUM_TOPICS_MANAGE)
            );
        }
        if let Some(cart) = registry.get("cart") {
            assert!(cart.permissions().is_empty());
        }
        if let Some(customer) = registry.get("customer") {
            assert!(
                customer
                    .permissions()
                    .contains(&Permission::CUSTOMERS_MANAGE)
            );
        }
        if let Some(product) = registry.get("product") {
            assert!(product.permissions().contains(&Permission::PRODUCTS_MANAGE));
        }
        if let Some(profiles) = registry.get("profiles") {
            assert!(
                profiles
                    .permissions()
                    .contains(&Permission::PROFILES_MANAGE)
            );
        }
        if let Some(region) = registry.get("region") {
            assert!(region.permissions().contains(&Permission::REGIONS_MANAGE));
        }
        if let Some(order) = registry.get("order") {
            assert!(order.permissions().contains(&Permission::ORDERS_MANAGE));
        }
        if let Some(payment) = registry.get("payment") {
            assert!(payment.permissions().contains(&Permission::PAYMENTS_MANAGE));
        }
        if let Some(fulfillment) = registry.get("fulfillment") {
            assert!(
                fulfillment
                    .permissions()
                    .contains(&Permission::FULFILLMENTS_MANAGE)
            );
        }
        if let Some(media) = registry.get("media") {
            assert!(
                media
                    .permissions()
                    .contains(&Permission::new(Resource::Media, Action::Manage))
            );
        }
        if let Some(pages) = registry.get("pages") {
            assert!(pages.permissions().contains(&Permission::PAGES_MANAGE));
        }
        if let Some(seo) = registry.get("seo") {
            assert!(seo.permissions().contains(&Permission::SEO_MANAGE));
        }
        if let Some(taxonomy) = registry.get("taxonomy") {
            assert!(
                taxonomy
                    .permissions()
                    .contains(&Permission::TAXONOMY_MANAGE)
            );
        }
        if let Some(workflow) = registry.get("workflow") {
            assert!(
                workflow
                    .permissions()
                    .contains(&Permission::WORKFLOWS_MANAGE)
            );
        }
        if let Some(alloy) = registry.get("alloy") {
            assert!(alloy.permissions().contains(&Permission::SCRIPTS_MANAGE));
        }
        if let Some(flex) = registry.get("flex") {
            assert!(
                flex.permissions()
                    .contains(&Permission::FLEX_SCHEMAS_MANAGE)
            );
            assert!(
                flex.permissions()
                    .contains(&Permission::FLEX_ENTRIES_MANAGE)
            );
        }
    }

    #[test]
    fn registry_dependencies_match_runtime_contract() {
        let registry = build_registry();
        let channel = registry.get("channel").expect("channel module");
        let outbox = registry.get("outbox").expect("outbox module");

        assert!(registry.is_core("channel"));
        assert!(registry.is_core("outbox"));
        assert!(channel.dependencies().is_empty());
        assert!(outbox.dependencies().is_empty());

        if let Some(cart) = registry.get("cart") {
            assert!(cart.dependencies().is_empty());
        }
        if let Some(customer) = registry.get("customer") {
            assert!(customer.dependencies().is_empty());
        }
        if let Some(product) = registry.get("product") {
            assert_eq!(product.dependencies(), &["taxonomy"]);
        }
        if let Some(profiles) = registry.get("profiles") {
            assert_eq!(
                profiles.dependencies(),
                &["media", "social_graph", "taxonomy"]
            );
        }
        if let Some(region) = registry.get("region") {
            assert!(region.dependencies().is_empty());
        }
        if let Some(payment) = registry.get("payment") {
            assert!(payment.dependencies().is_empty());
        }
        if let Some(fulfillment) = registry.get("fulfillment") {
            assert!(fulfillment.dependencies().is_empty());
        }
        if let Some(commerce) = registry.get("commerce") {
            assert_eq!(
                commerce.dependencies(),
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
                    "fulfillment"
                ]
            );
        }
        if let Some(blog) = registry.get("blog") {
            assert_eq!(
                blog.dependencies(),
                &["content", "taxonomy", "outbox", "channel"]
            );
        }
        if let Some(forum) = registry.get("forum") {
            assert_eq!(forum.dependencies(), &["content", "media", "taxonomy"]);
        }
        if let Some(pages) = registry.get("pages") {
            assert_eq!(pages.dependencies(), &["content", "page_builder"]);
        }
        if let Some(seo) = registry.get("seo") {
            assert_eq!(seo.dependencies(), &["content"]);
        }
        if let Some(taxonomy) = registry.get("taxonomy") {
            assert_eq!(taxonomy.dependencies(), &["content"]);
        }
        if let Some(workflow) = registry.get("workflow") {
            assert!(workflow.dependencies().is_empty());
        }
        if let Some(alloy) = registry.get("alloy") {
            assert!(alloy.dependencies().is_empty());
        }
        if let Some(flex) = registry.get("flex") {
            assert!(flex.dependencies().is_empty());
        }
    }

    #[test]
    #[serial]
    fn runtime_registry_dependencies_match_modules_manifest() {
        let manifest = ManifestManager::load().expect("modules.toml must load");
        let registry = build_registry();

        // When running in a partial/scoped build without all module features enabled,
        // skip full manifest registry validation.
        if registry.modules().count() < manifest.modules.len() {
            return;
        }

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
