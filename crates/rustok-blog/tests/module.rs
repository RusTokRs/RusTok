//! Module metadata tests

use rustok_blog::BlogModule;
use rustok_core::permissions::{Action, Resource};
use rustok_core::{MigrationSource, RusToKModule};

#[test]
fn module_metadata() {
    let module = BlogModule;

    assert_eq!(module.slug(), "blog");
    assert_eq!(module.name(), "Blog");
    assert_eq!(module.description(), "Posts, Comments, Categories, Tags");
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn module_has_permissions() {
    let module = BlogModule;
    let permissions = module.permissions();

    // Should have permissions for posts
    assert!(
        !permissions.is_empty(),
        "Module should have permissions defined"
    );

    // Check specific permissions exist
    let has_posts_create = permissions
        .iter()
        .any(|p| p.resource == Resource::Posts && p.action == Action::Create);
    assert!(has_posts_create, "Should have posts:create permission");

    let has_posts_publish = permissions
        .iter()
        .any(|p| p.resource == Resource::Posts && p.action == Action::Publish);
    assert!(has_posts_publish, "Should have posts:publish permission");

    let has_comments_moderate = permissions
        .iter()
        .any(|p| p.resource == Resource::Comments && p.action == Action::Moderate);
    assert!(
        has_comments_moderate,
        "Should have comments:moderate permission"
    );
}

#[test]
fn module_migrations_empty() {
    let module = BlogModule;

    // Blog module uses content module tables, so no own migrations
    assert!(
        module.migrations().is_empty(),
        "Blog module should not have own migrations"
    );
}

#[test]
fn module_slug_is_stable() {
    let module = BlogModule;

    // Slug should never change as it's used for configuration
    assert_eq!(module.slug(), "blog");
}

#[test]
fn module_permissions_cover_all_resources() {
    let module = BlogModule;
    let permissions = module.permissions();

    let resources: std::collections::HashSet<_> = permissions.iter().map(|p| p.resource).collect();

    // Should cover all blog-related resources
    assert!(
        resources.contains(&Resource::Posts),
        "Should have Posts resource"
    );
    assert!(
        resources.contains(&Resource::Comments),
        "Should have Comments resource"
    );
    assert!(
        resources.contains(&Resource::Categories),
        "Should have Categories resource"
    );
    assert!(
        resources.contains(&Resource::Tags),
        "Should have Tags resource"
    );
}
