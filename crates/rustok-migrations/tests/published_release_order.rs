use rustok_migrations::Migrator;
use sea_orm_migration::MigratorTrait;

#[test]
fn migrator_preserves_append_only_migration_tail() {
    let names = Migrator::migrations()
        .into_iter()
        .map(|migration| migration.name().to_string())
        .collect::<Vec<_>>();
    let published_tail = [
        "m20260728_000001_create_consumer_poison_receipts",
        "m20260803_000017_add_forum_topic_canonical_resolution",
        "m20260803_000009_add_blog_comments_audit_canonical_handoff",
        "m20260803_000001_canonicalize_artifact_permissions",
    ];

    let positions = published_tail
        .iter()
        .map(|name| {
            names
                .iter()
                .position(|candidate| candidate == name)
                .unwrap_or_else(|| {
                    panic!("published migration {name} is missing from the platform plan")
                })
        })
        .collect::<Vec<_>>();

    for (names, positions) in published_tail.windows(2).zip(positions.windows(2)) {
        assert!(
            positions[0] < positions[1],
            "published migration order changed: {} must remain before {}",
            names[0],
            names[1]
        );
    }
}
