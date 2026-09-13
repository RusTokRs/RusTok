use rustok_product_relations::dto::{
    CreateProductRelationInput, RelationType, ReorderProductRelationsInput,
    UpdateProductRelationInput,
};
use rustok_product_relations::error::ProductRelationError;
use rustok_product_relations::ports::ProductRelationsPort;
use rustok_product_relations::services::ProductRelationService;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use uuid::Uuid;

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory sqlite");

    db.execute_unprepared(
        r#"
CREATE TABLE product_relations (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    related_product_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (tenant_id, product_id, related_product_id, relation_type)
);
"#,
    )
    .await
    .expect("Failed to create product_relations table");

    db
}

#[tokio::test]
async fn rejects_self_relation() {
    let db = setup_test_db().await;
    let service = ProductRelationService::new(db);

    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();

    let err = service
        .create_relation(
            tenant_id,
            None,
            CreateProductRelationInput {
                product_id,
                related_product_id: product_id,
                relation_type: RelationType::CrossSell,
                position: None,
                metadata: None,
            },
        )
        .await
        .expect_err("Self-relation must be rejected");

    match err {
        ProductRelationError::SelfRelationNotAllowed(id) => assert_eq!(id, product_id),
        other => panic!("Unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn creates_and_queries_relations() {
    let db = setup_test_db().await;
    let service = ProductRelationService::new(db);

    let tenant_id = Uuid::new_v4();
    let product_a = Uuid::new_v4();
    let product_b = Uuid::new_v4();
    let product_c = Uuid::new_v4();

    // Add cross-sell relation A -> B
    let rel_ab = service
        .create_relation(
            tenant_id,
            None,
            CreateProductRelationInput {
                product_id: product_a,
                related_product_id: product_b,
                relation_type: RelationType::CrossSell,
                position: None,
                metadata: Some(serde_json::json!({"discount_percent": 10})),
            },
        )
        .await
        .expect("Failed to create relation A -> B");

    assert_eq!(rel_ab.product_id, product_a);
    assert_eq!(rel_ab.related_product_id, product_b);
    assert_eq!(rel_ab.relation_type, RelationType::CrossSell);
    assert_eq!(rel_ab.position, 0);

    // Add up-sell relation A -> C
    let rel_ac = service
        .create_relation(
            tenant_id,
            None,
            CreateProductRelationInput {
                product_id: product_a,
                related_product_id: product_c,
                relation_type: RelationType::UpSell,
                position: None,
                metadata: None,
            },
        )
        .await
        .expect("Failed to create relation A -> C");

    assert_eq!(rel_ac.relation_type, RelationType::UpSell);

    // Query all relations for product A
    let all = service
        .list_relations(tenant_id, product_a, None)
        .await
        .expect("Failed to list relations");
    assert_eq!(all.len(), 2);

    // Filter by type CrossSell
    let cross = service
        .list_relations(tenant_id, product_a, Some(RelationType::CrossSell))
        .await
        .expect("Failed to list cross-sell");
    assert_eq!(cross.len(), 1);
    assert_eq!(cross[0].id, rel_ab.id);

    // Reverse lookup for product B
    let reverse = service
        .list_reverse_relations(tenant_id, product_b, None)
        .await
        .expect("Failed to list reverse relations");
    assert_eq!(reverse.len(), 1);
    assert_eq!(reverse[0].product_id, product_a);
}

#[tokio::test]
async fn rejects_duplicate_relation_pair_and_type() {
    let db = setup_test_db().await;
    let service = ProductRelationService::new(db);

    let tenant_id = Uuid::new_v4();
    let product_a = Uuid::new_v4();
    let product_b = Uuid::new_v4();

    service
        .create_relation(
            tenant_id,
            None,
            CreateProductRelationInput {
                product_id: product_a,
                related_product_id: product_b,
                relation_type: RelationType::Accessory,
                position: None,
                metadata: None,
            },
        )
        .await
        .expect("Initial relation creation failed");

    let err = service
        .create_relation(
            tenant_id,
            None,
            CreateProductRelationInput {
                product_id: product_a,
                related_product_id: product_b,
                relation_type: RelationType::Accessory,
                position: None,
                metadata: None,
            },
        )
        .await
        .expect_err("Duplicate relation must fail");

    match err {
        ProductRelationError::RelationAlreadyExists { relation_type, .. } => {
            assert_eq!(relation_type, "accessory");
        }
        other => panic!("Unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn updates_deletes_and_reorders_relations() {
    let db = setup_test_db().await;
    let service = ProductRelationService::new(db);

    let tenant_id = Uuid::new_v4();
    let product_a = Uuid::new_v4();
    let product_b = Uuid::new_v4();
    let product_c = Uuid::new_v4();

    let rel1 = service
        .create_relation(
            tenant_id,
            None,
            CreateProductRelationInput {
                product_id: product_a,
                related_product_id: product_b,
                relation_type: RelationType::Related,
                position: Some(0),
                metadata: None,
            },
        )
        .await
        .unwrap();

    let rel2 = service
        .create_relation(
            tenant_id,
            None,
            CreateProductRelationInput {
                product_id: product_a,
                related_product_id: product_c,
                relation_type: RelationType::Related,
                position: Some(1),
                metadata: None,
            },
        )
        .await
        .unwrap();

    // Update metadata on rel1
    let updated = service
        .update_relation(
            tenant_id,
            None,
            rel1.id,
            UpdateProductRelationInput {
                position: None,
                metadata: Some(serde_json::json!({"note": "hand-picked"})),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.metadata, serde_json::json!({"note": "hand-picked"}));

    // Reorder so that rel2 is first and rel1 is second
    let reordered = service
        .reorder_relations(
            tenant_id,
            None,
            ReorderProductRelationsInput {
                product_id: product_a,
                relation_type: RelationType::Related,
                ordered_relation_ids: vec![rel2.id, rel1.id],
            },
        )
        .await
        .unwrap();

    assert_eq!(reordered[0].id, rel2.id);
    assert_eq!(reordered[0].position, 0);
    assert_eq!(reordered[1].id, rel1.id);
    assert_eq!(reordered[1].position, 1);

    // Delete rel1
    service.delete_relation(tenant_id, None, rel1.id).await.unwrap();

    let remaining = service
        .list_relations(tenant_id, product_a, Some(RelationType::Related))
        .await
        .unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, rel2.id);
}
