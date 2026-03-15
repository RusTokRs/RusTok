use rustok_core::permissions::{Action, Resource};
use rustok_core::{MigrationSource, RusToKModule};
use rustok_pages::PagesModule;

#[test]
fn module_metadata() {
    let module = PagesModule;
    assert_eq!(module.slug(), "pages");
    assert_eq!(module.name(), "Pages");
    assert_eq!(module.description(), "Static pages, blocks and menus");
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn module_permissions() {
    let module = PagesModule;
    let permissions = module.permissions();

    // Check pages permissions exist
    assert!(permissions
        .iter()
        .any(|p| { p.resource == Resource::Pages && p.action == Action::Create }));
    assert!(permissions
        .iter()
        .any(|p| { p.resource == Resource::Pages && p.action == Action::Publish }));

    // Check nodes (blocks) permissions exist
    assert!(permissions
        .iter()
        .any(|p| { p.resource == Resource::Nodes && p.action == Action::Create }));
}

#[test]
fn module_migrations_empty() {
    let module = PagesModule;
    assert!(module.migrations().is_empty());
}
