use rustok_core::{MigrationSource, RusToKModule};
use rustok_forum::ForumModule;

#[test]
fn module_metadata() {
    let module = ForumModule;
    assert_eq!(module.slug(), "forum");
    assert_eq!(module.name(), "Forum");
    assert_eq!(
        module.description(),
        "Forum categories, topics, replies, and moderation workflows"
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn module_migrations_empty() {
    let module = ForumModule;
    assert!(module.migrations().is_empty());
}
