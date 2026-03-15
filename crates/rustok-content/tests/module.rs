use rustok_content::ContentModule;
use rustok_core::{Action, Permission, Resource, RusToKModule};

#[test]
fn module_metadata() {
    let module = ContentModule;
    assert_eq!(module.slug(), "content");
    assert_eq!(module.name(), "Content");
    assert_eq!(
        module.description(),
        "Core CMS Module (Nodes, Bodies, Categories)"
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn module_permissions_include_orchestration_resources() {
    let module = ContentModule;
    let permissions = module.permissions();

    let expected = [
        Permission::new(Resource::ForumTopics, Action::Create),
        Permission::new(Resource::ForumTopics, Action::Read),
        Permission::new(Resource::ForumTopics, Action::Update),
        Permission::new(Resource::ForumTopics, Action::Delete),
        Permission::new(Resource::ForumTopics, Action::List),
        Permission::new(Resource::ForumTopics, Action::Moderate),
        Permission::new(Resource::BlogPosts, Action::Create),
        Permission::new(Resource::BlogPosts, Action::Read),
        Permission::new(Resource::BlogPosts, Action::Update),
        Permission::new(Resource::BlogPosts, Action::Delete),
        Permission::new(Resource::BlogPosts, Action::List),
        Permission::new(Resource::BlogPosts, Action::Moderate),
    ];

    for permission in expected {
        assert!(
            permissions.contains(&permission),
            "missing expected permission: {permission}"
        );
    }
}

#[test]
fn module_permissions_cover_orchestration_runtime_checks() {
    let module = ContentModule;
    let permissions = module.permissions();

    // ensure_scope(...) checks in ContentOrchestrationService:
    // - ForumTopics:Moderate + BlogPosts:Create for promote_topic_to_post
    // - BlogPosts:Moderate + ForumTopics:Create for demote_post_to_topic
    // - ForumTopics:Moderate for split_topic/merge_topics
    let runtime_checked = [
        Permission::new(Resource::ForumTopics, Action::Moderate),
        Permission::new(Resource::ForumTopics, Action::Create),
        Permission::new(Resource::BlogPosts, Action::Moderate),
        Permission::new(Resource::BlogPosts, Action::Create),
    ];

    for permission in runtime_checked {
        assert!(
            permissions.contains(&permission),
            "orchestration runtime check is not declared in module permissions: {permission}"
        );
    }
}

#[test]
fn module_migrations_empty() {
    let module = ContentModule;
    assert!(module.migrations().is_empty());
}
