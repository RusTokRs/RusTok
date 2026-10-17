use rustok_outbox::SysEventsMigration;
use rustok_product_relations::dto::{
    CreateProductRelationInput, RelationType, ReorderProductRelationsInput,
    UpdateProductRelationInput,
};
use rustok_product_relations::error::ProductRelationError;
use rustok_product_relations::ports::ProductRelationsPort;
use rustok_product_relations::services::{ProductRelationCommandContext, ProductRelationService};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory sqlite");

    db.execute_unprepared("PRAGMA foreign_keys = ON;")
        .await
        .expect("SQLite foreign-key enforcement must be enabled");
    SysEventsMigration
        .up(&SchemaManager::new(&db))
        .await
        .expect("owner receipt schema must be available in relation tests");
    db.execute_unprepared(
        r#"
CREATE TABLE products (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    seller_id TEXT,
    vendor TEXT,
    product_type TEXT,
    shipping_profile_slug TEXT,
    primary_category_id TEXT,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    published_at TEXT,
    revision INTEGER NOT NULL DEFAULT 1,
    UNIQUE (tenant_id, id)
);

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
    FOREIGN KEY (tenant_id, product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, related_product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE,
    CHECK (product_id <> related_product_id),
    CHECK (position >= 0),
    UNIQUE (tenant_id, product_id, related_product_id, relation_type),
    UNIQUE (tenant_id, product_id, relation_type, position)
);
"#,
    )
    .await
    .expect("Failed to create relation test tables");

    db
}

async fn insert_test_product(db: &DatabaseConnection, tenant_id: Uuid, product_id: Uuid) {
    use rustok_product::entities::product;
    use sea_orm::{ActiveModelTrait, Set};

    product::ActiveModel {
        id: Set(product_id),
        tenant_id: Set(tenant_id),
        status: Set(product::ProductStatus::Active),
        metadata: Set(serde_json::json!({})),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        revision: Set(1),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("Failed to insert relation test product");
}

#[tokio::test]
async fn rejects_self_relation() {
    let db = setup_test_db().await;
    let service = ProductRelationService::new(db.clone());

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
    let service = ProductRelationService::new(db.clone());

    let tenant_id = Uuid::new_v4();
    let product_a = Uuid::new_v4();
    let product_b = Uuid::new_v4();
    let product_c = Uuid::new_v4();
    insert_test_product(&db, tenant_id, product_a).await;
    insert_test_product(&db, tenant_id, product_b).await;
    insert_test_product(&db, tenant_id, product_c).await;

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
    let service = ProductRelationService::new(db.clone());

    let tenant_id = Uuid::new_v4();
    let product_a = Uuid::new_v4();
    let product_b = Uuid::new_v4();
    insert_test_product(&db, tenant_id, product_a).await;
    insert_test_product(&db, tenant_id, product_b).await;

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
async fn maintains_complete_unique_relation_ordering() {
    let db = setup_test_db().await;
    let service = ProductRelationService::new(db.clone());

    let tenant_id = Uuid::new_v4();
    let product_a = Uuid::new_v4();
    let product_b = Uuid::new_v4();
    let product_c = Uuid::new_v4();
    let product_d = Uuid::new_v4();
    for product_id in [product_a, product_b, product_c, product_d] {
        insert_test_product(&db, tenant_id, product_id).await;
    }

    let create = |related_product_id| CreateProductRelationInput {
        product_id: product_a,
        related_product_id,
        relation_type: RelationType::Related,
        position: None,
        metadata: None,
    };
    let rel1 = service
        .create_relation(tenant_id, None, create(product_b))
        .await
        .unwrap();
    let rel2 = service
        .create_relation(tenant_id, None, create(product_c))
        .await
        .unwrap();
    let rel3 = service
        .create_relation(tenant_id, None, create(product_d))
        .await
        .unwrap();

    // Moving an item uses a complete normalized assignment, so the unique position constraint is
    // never violated during the swap.
    let moved = service
        .update_relation(
            tenant_id,
            None,
            rel3.id,
            UpdateProductRelationInput {
                position: Some(0),
                metadata: Some(serde_json::json!({"note": "hand-picked"})),
            },
        )
        .await
        .unwrap();
    assert_eq!(moved.position, 0);
    assert_eq!(moved.metadata, serde_json::json!({"note": "hand-picked"}));

    let partial_error = service
        .reorder_relations(
            tenant_id,
            None,
            ReorderProductRelationsInput {
                product_id: product_a,
                relation_type: RelationType::Related,
                ordered_relation_ids: vec![rel3.id, rel2.id],
            },
        )
        .await
        .expect_err("partial reorder must be rejected");
    assert!(matches!(
        partial_error,
        ProductRelationError::IncompleteReorder {
            expected: 3,
            received: 2
        }
    ));

    let duplicate_error = service
        .reorder_relations(
            tenant_id,
            None,
            ReorderProductRelationsInput {
                product_id: product_a,
                relation_type: RelationType::Related,
                ordered_relation_ids: vec![rel3.id, rel3.id, rel1.id],
            },
        )
        .await
        .expect_err("duplicate relation IDs must be rejected");
    assert!(matches!(
        duplicate_error,
        ProductRelationError::DuplicateRelationInReorder(relation_id) if relation_id == rel3.id
    ));

    let reordered = service
        .reorder_relations(
            tenant_id,
            None,
            ReorderProductRelationsInput {
                product_id: product_a,
                relation_type: RelationType::Related,
                ordered_relation_ids: vec![rel2.id, rel1.id, rel3.id],
            },
        )
        .await
        .unwrap();
    assert_eq!(
        reordered
            .iter()
            .map(|relation| relation.id)
            .collect::<Vec<_>>(),
        vec![rel2.id, rel1.id, rel3.id]
    );
    assert_eq!(
        reordered
            .iter()
            .map(|relation| relation.position)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    // Deletion compacts the remaining group rather than leaving a gap.
    service
        .delete_relation(tenant_id, None, rel1.id)
        .await
        .unwrap();
    let remaining = service
        .list_relations(tenant_id, product_a, Some(RelationType::Related))
        .await
        .unwrap();
    assert_eq!(
        remaining
            .iter()
            .map(|relation| relation.id)
            .collect::<Vec<_>>(),
        vec![rel2.id, rel3.id]
    );
    assert_eq!(
        remaining
            .iter()
            .map(|relation| relation.position)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[tokio::test]
async fn idempotent_relation_commands_replay_each_supported_response() {
    let db = setup_test_db().await;
    let service = ProductRelationService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    let product_a = Uuid::new_v4();
    let product_b = Uuid::new_v4();
    let product_c = Uuid::new_v4();
    for product_id in [product_a, product_b, product_c] {
        insert_test_product(&db, tenant_id, product_id).await;
    }

    let first_input = CreateProductRelationInput {
        product_id: product_a,
        related_product_id: product_b,
        relation_type: RelationType::Related,
        position: None,
        metadata: Some(serde_json::json!({"origin": "receipt-test"})),
    };
    let first = service
        .create_relation_idempotent(
            ProductRelationCommandContext::new(tenant_id, None, "relation-create-first"),
            first_input.clone(),
        )
        .await
        .expect("initial receipt-bound relation create must succeed");
    let first_replay = service
        .create_relation_idempotent(
            ProductRelationCommandContext::new(tenant_id, None, "relation-create-first"),
            first_input,
        )
        .await
        .expect("same create key must replay its response");
    assert_eq!(first_replay, first);

    let second = service
        .create_relation_idempotent(
            ProductRelationCommandContext::new(tenant_id, None, "relation-create-second"),
            CreateProductRelationInput {
                product_id: product_a,
                related_product_id: product_c,
                relation_type: RelationType::Related,
                position: None,
                metadata: None,
            },
        )
        .await
        .expect("second receipt-bound relation create must succeed");

    let update_input = UpdateProductRelationInput {
        position: Some(1),
        metadata: Some(serde_json::json!({"origin": "updated"})),
    };
    let updated = service
        .update_relation_idempotent(
            ProductRelationCommandContext::new(tenant_id, None, "relation-update"),
            first.id,
            update_input.clone(),
        )
        .await
        .expect("receipt-bound update must succeed");
    let updated_replay = service
        .update_relation_idempotent(
            ProductRelationCommandContext::new(tenant_id, None, "relation-update"),
            first.id,
            update_input,
        )
        .await
        .expect("same update key must replay its response");
    assert_eq!(updated_replay, updated);

    let reorder_input = ReorderProductRelationsInput {
        product_id: product_a,
        relation_type: RelationType::Related,
        ordered_relation_ids: vec![first.id, second.id],
    };
    let reordered = service
        .reorder_relations_idempotent(
            ProductRelationCommandContext::new(tenant_id, None, "relation-reorder"),
            reorder_input.clone(),
        )
        .await
        .expect("receipt-bound reorder must succeed");
    let reordered_replay = service
        .reorder_relations_idempotent(
            ProductRelationCommandContext::new(tenant_id, None, "relation-reorder"),
            reorder_input,
        )
        .await
        .expect("same reorder key must replay its response");
    assert_eq!(reordered_replay, reordered);

    service
        .delete_relation_idempotent(
            ProductRelationCommandContext::new(tenant_id, None, "relation-delete"),
            first.id,
        )
        .await
        .expect("receipt-bound delete must succeed");
    service
        .delete_relation_idempotent(
            ProductRelationCommandContext::new(tenant_id, None, "relation-delete"),
            first.id,
        )
        .await
        .expect("same delete key must replay completion after row removal");

    let remaining = service
        .list_relations(tenant_id, product_a, Some(RelationType::Related))
        .await
        .expect("remaining relation list must be readable");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, second.id);
}
