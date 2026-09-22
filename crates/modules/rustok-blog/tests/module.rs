//! Module metadata tests

use rustok_api::{Action, Resource};
use rustok_blog::BlogModule;
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

    assert!(
        permissions
            .iter()
            .any(|p| p.resource == Resource::BlogPosts && p.action == Action::Create)
    );
    assert!(
        permissions
            .iter()
            .any(|p| p.resource == Resource::BlogPosts && p.action == Action::Publish)
    );
    assert!(
        permissions
            .iter()
            .any(|p| p.resource == Resource::BlogPosts && p.action == Action::Manage)
    );
    assert!(
        permissions
            .iter()
            .any(|p| p.resource == Resource::BlogCategories && p.action == Action::Create)
    );
    assert!(
        permissions
            .iter()
            .any(|p| p.resource == Resource::BlogCategories && p.action == Action::Manage)
    );
    assert!(
        !permissions
            .iter()
            .any(|p| p.resource == Resource::Categories)
    );
}

#[test]
fn module_has_owned_migrations() {
    let module = BlogModule;
    assert!(
        !module.migrations().is_empty(),
        "Blog module should expose its own migrations"
    );
}

#[test]
fn module_slug_is_stable() {
    let module = BlogModule;
    assert_eq!(module.slug(), "blog");
}

#[test]
fn module_permissions_cover_all_resources() {
    let module = BlogModule;
    let permissions = module.permissions();
    let resources: std::collections::HashSet<_> = permissions.iter().map(|p| p.resource).collect();

    assert!(resources.contains(&Resource::BlogPosts));
    assert!(resources.contains(&Resource::BlogCategories));
    assert!(!resources.contains(&Resource::Categories));
}
