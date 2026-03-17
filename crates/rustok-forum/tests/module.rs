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
fn module_has_migrations() {
    let module = ForumModule;
    let migrations = module.migrations();
    assert!(!migrations.is_empty(), "ForumModule must own its migrations");
    let names: Vec<_> = migrations.iter().map(|m| m.name()).collect();
    assert!(names.iter().any(|n| n.contains("topic_field_definitions")));
}
