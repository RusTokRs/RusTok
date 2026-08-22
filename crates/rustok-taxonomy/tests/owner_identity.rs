use rustok_taxonomy::{TaxonomyTermKind, taxonomy_term_identity_exists};
use sea_orm::{ConnectionTrait, Database};
use uuid::Uuid;

#[tokio::test]
async fn owner_identity_is_bounded_by_tenant_and_term_kind() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite Taxonomy fixture should connect");
    db.execute_unprepared(
        "CREATE TABLE taxonomy_terms (\
            id TEXT PRIMARY KEY NOT NULL, \
            tenant_id TEXT NOT NULL, \
            kind TEXT NOT NULL, \
            scope_type TEXT NOT NULL, \
            scope_value TEXT NOT NULL, \
            canonical_key TEXT NOT NULL, \
            revision INTEGER NOT NULL, \
            created_at TEXT NOT NULL, \
            updated_at TEXT NOT NULL\
        )",
    )
    .await
    .expect("taxonomy_terms fixture should create");

    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let tag_id = Uuid::new_v4();
    let timestamp = "2026-08-22T00:00:00+00:00";

    db.execute_unprepared(&format!(
        "INSERT INTO taxonomy_terms \
         (id, tenant_id, kind, scope_type, scope_value, canonical_key, revision, created_at, updated_at) \
         VALUES ('{category_id}', '{tenant_id}', 'category', 'global', '', 'category', 1, '{timestamp}', '{timestamp}')"
    ))
    .await
    .expect("Category fixture should insert");
    db.execute_unprepared(&format!(
        "INSERT INTO taxonomy_terms \
         (id, tenant_id, kind, scope_type, scope_value, canonical_key, revision, created_at, updated_at) \
         VALUES ('{tag_id}', '{tenant_id}', 'tag', 'global', '', 'tag', 1, '{timestamp}', '{timestamp}')"
    ))
    .await
    .expect("Tag fixture should insert");

    assert!(
        taxonomy_term_identity_exists(&db, tenant_id, TaxonomyTermKind::Category, category_id)
            .await
            .expect("Category identity lookup should succeed")
    );
    assert!(
        !taxonomy_term_identity_exists(&db, other_tenant_id, TaxonomyTermKind::Category, category_id)
            .await
            .expect("foreign tenant lookup should succeed")
    );
    assert!(
        !taxonomy_term_identity_exists(&db, tenant_id, TaxonomyTermKind::Category, tag_id)
            .await
            .expect("wrong kind lookup should succeed")
    );
    assert!(
        !taxonomy_term_identity_exists(
            &db,
            tenant_id,
            TaxonomyTermKind::Category,
            Uuid::new_v4(),
        )
        .await
        .expect("missing identity lookup should succeed")
    );
}
