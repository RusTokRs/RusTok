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

    assert!(
        !permissions.is_empty(),
        "Module should have permissions defined"
    );

    let has_posts_create = permissions
        .iter()
        .any(|p| p.resource == Resource::BlogPosts && p.action == Action::Create);
    assert!(has_posts_create, "Should have blog_posts:create permission");

    let has_posts_publish = permissions
        .iter()
        .any(|p| p.resource == Resource::BlogPosts && p.action == Action::Publish);
    assert!(
        has_posts_publish,
        "Should have blog_posts:publish permission"
    );

    let has_posts_manage = permissions
        .iter()
        .any(|p| p.resource == Resource::BlogPosts && p.action == Action::Manage);
    assert!(has_posts_manage, "Should have blog_posts:manage permission");
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

    assert!(
        resources.contains(&Resource::BlogPosts),
        "Should have BlogPosts resource"
    );
}
