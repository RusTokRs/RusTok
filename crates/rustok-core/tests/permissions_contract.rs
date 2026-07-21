use rustok_api::{Action, Permission, Resource};
use std::str::FromStr;

#[test]
fn all_public_resources_display_parse_roundtrip() {
    let resources = [
        Resource::Users,
        Resource::Tenants,
        Resource::Modules,
        Resource::Settings,
        Resource::FlexSchemas,
        Resource::FlexEntries,
        Resource::Products,
        Resource::Categories,
        Resource::Orders,
        Resource::Customers,
        Resource::Profiles,
        Resource::Groups,
        Resource::Regions,
        Resource::Payments,
        Resource::Fulfillments,
        Resource::Inventory,
        Resource::Discounts,
        Resource::MarketplaceSellers,
        Resource::MarketplaceListings,
        Resource::Posts,
        Resource::Pages,
        Resource::Nodes,
        Resource::Media,
        Resource::Seo,
        Resource::Comments,
        Resource::Tags,
        Resource::Taxonomy,
        Resource::Analytics,
        Resource::Logs,
        Resource::Webhooks,
        Resource::BlogPosts,
        Resource::BlogCategories,
        Resource::ForumCategories,
        Resource::ForumTopics,
        Resource::ForumReplies,
        Resource::Scripts,
        Resource::Mcp,
        Resource::AiProviders,
        Resource::AiTaskProfiles,
        Resource::AiSessions,
        Resource::AiRuns,
        Resource::AiApprovals,
        Resource::AiRouter,
        Resource::AiTextTasks,
        Resource::AiImageTasks,
        Resource::AiCodeTasks,
        Resource::AiAlloyTasks,
        Resource::AiMultimodalTasks,
        Resource::Workflows,
        Resource::WorkflowExecutions,
    ];

    for resource in resources {
        assert_eq!(Resource::from_str(&resource.to_string()).unwrap(), resource);
    }
}

#[test]
fn all_public_actions_display_parse_roundtrip() {
    let actions = [
        Action::Create,
        Action::Read,
        Action::Update,
        Action::Delete,
        Action::List,
        Action::Export,
        Action::Import,
        Action::Manage,
        Action::Publish,
        Action::Moderate,
        Action::Execute,
        Action::Run,
        Action::Cancel,
        Action::Resolve,
        Action::Override,
    ];

    for action in actions {
        assert_eq!(Action::from_str(&action.to_string()).unwrap(), action);
    }
    assert_eq!(Action::from_str("*").unwrap(), Action::Manage);
}

#[test]
fn permission_parser_splits_on_last_colon_for_namespaced_resources() {
    let permission = Permission::from_str("ai:tasks:text:run").unwrap();
    assert_eq!(permission.resource, Resource::AiTextTasks);
    assert_eq!(permission.action, Action::Run);
    assert_eq!(permission.to_string(), "ai:tasks:text:run");
}

#[test]
fn permission_parser_reports_malformed_contracts() {
    assert!(Permission::from_str("products").is_err());
    assert!(Permission::from_str("unknown:read").is_err());
    assert!(Permission::from_str("products:unknown").is_err());
}

#[test]
fn permission_constants_match_canonical_strings() {
    let cases = [
        (Permission::USERS_MANAGE, "users:manage"),
        (Permission::PRODUCTS_READ, "products:read"),
        (Permission::SEO_GENERATE, "seo:execute"),
        (Permission::BLOG_POSTS_PUBLISH, "blog_posts:publish"),
        (
            Permission::BLOG_CATEGORIES_MANAGE,
            "blog_categories:manage",
        ),
        (
            Permission::FORUM_TOPICS_MODERATE,
            "forum_topics:moderate",
        ),
        (Permission::AI_ROUTER_OVERRIDE, "ai:router:override"),
        (
            Permission::AI_TASKS_MULTIMODAL_RUN,
            "ai:tasks:multimodal:run",
        ),
        (Permission::WORKFLOWS_EXECUTE, "workflows:execute"),
        (
            Permission::WORKFLOW_EXECUTIONS_LIST,
            "workflow_executions:list",
        ),
    ];

    for (permission, expected) in cases {
        assert_eq!(permission.to_string(), expected);
        assert_eq!(Permission::from_str(expected).unwrap(), permission);
    }
}

#[test]
fn catalog_and_blog_category_permissions_are_distinct() {
    assert_ne!(
        Permission::new(Resource::Categories, Action::Update),
        Permission::BLOG_CATEGORIES_UPDATE
    );
    assert_eq!(
        Permission::from_str("blog_categories:update").unwrap(),
        Permission::BLOG_CATEGORIES_UPDATE
    );
}
