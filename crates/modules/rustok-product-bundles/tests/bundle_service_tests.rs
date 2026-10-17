use rustok_product_bundles::dto::{
    BundleFilter, BundleItemInput, BundleTranslationInput, CreateBundleInput, UpdateBundleInput,
};
use rustok_product_bundles::error::BundleError;
use rustok_product_bundles::ports::BundlePort;
use rustok_product_bundles::services::BundleService;
use sea_orm::prelude::Decimal;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use uuid::Uuid;

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory sqlite");

    db.execute_unprepared("PRAGMA foreign_keys = ON;")
        .await
        .expect("SQLite foreign-key enforcement must be enabled");
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

CREATE TABLE product_variants (
    id TEXT PRIMARY KEY,
    product_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    sku TEXT,
    barcode TEXT,
    shipping_profile_slug TEXT,
    ean TEXT,
    upc TEXT,
    inventory_policy TEXT NOT NULL DEFAULT 'deny',
    inventory_management TEXT NOT NULL DEFAULT 'manual',
    inventory_quantity INTEGER NOT NULL DEFAULT 0,
    weight REAL,
    weight_unit TEXT,
    combination_identity TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (tenant_id, id)
);

CREATE TABLE product_bundles (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    bundle_product_id TEXT,
    slug TEXT NOT NULL,
    bundle_type TEXT NOT NULL DEFAULT 'fixed',
    status TEXT NOT NULL DEFAULT 'active',
    discount_type TEXT NOT NULL DEFAULT 'none',
    discount_value REAL NOT NULL DEFAULT 0,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (tenant_id, slug),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, bundle_product_id),
    FOREIGN KEY (tenant_id, bundle_product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE,
    CHECK (bundle_type IN ('fixed', 'flexible')),
    CHECK (status IN ('draft', 'active', 'archived')),
    CHECK (discount_type IN ('none', 'percentage', 'fixed_amount')),
    CHECK (
        discount_value >= 0
        AND (discount_type <> 'none' OR discount_value = 0)
        AND (discount_type <> 'percentage' OR discount_value <= 100)
    )
);

CREATE TABLE product_bundle_translations (
    id TEXT PRIMARY KEY,
    bundle_id TEXT NOT NULL REFERENCES product_bundles(id) ON DELETE CASCADE,
    locale TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (bundle_id, locale)
);

CREATE TABLE product_bundle_items (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    bundle_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    variant_id TEXT,
    quantity INTEGER NOT NULL DEFAULT 1,
    is_optional INTEGER NOT NULL DEFAULT 0,
    discount_rate REAL,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    FOREIGN KEY (tenant_id, bundle_id)
        REFERENCES product_bundles (tenant_id, id)
        ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, variant_id)
        REFERENCES product_variants (tenant_id, id)
        ON DELETE CASCADE,
    UNIQUE (tenant_id, bundle_id, position),
    CHECK (quantity >= 1),
    CHECK (position >= 0),
    CHECK (discount_rate IS NULL OR (discount_rate >= 0 AND discount_rate <= 1))
);
"#,
    )
    .await
    .expect("Failed to create bundle tables");

    db
}

async fn insert_test_product(db: &DatabaseConnection, tenant_id: Uuid, product_id: Uuid) {
    use rustok_product::entities::product;
    use sea_orm::{ActiveModelTrait, Set};

    let prod = product::ActiveModel {
        id: Set(product_id),
        tenant_id: Set(tenant_id),
        status: Set(product::ProductStatus::Active),
        metadata: Set(serde_json::json!({})),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        revision: Set(1),
        ..Default::default()
    };
    prod.insert(db)
        .await
        .expect("Failed to insert test product");
}

#[tokio::test]
async fn creates_and_queries_bundle_with_translations_and_items() {
    let db = setup_test_db().await;
    let service = BundleService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    let product_id_1 = Uuid::new_v4();
    let product_id_2 = Uuid::new_v4();

    insert_test_product(&db, tenant_id, product_id_1).await;
    insert_test_product(&db, tenant_id, product_id_2).await;

    let created = service
        .create_bundle(
            tenant_id,
            CreateBundleInput {
                bundle_product_id: None,
                slug: "starter-kit".to_string(),
                bundle_type: Some("fixed".to_string()),
                status: Some("active".to_string()),
                discount_type: Some("percentage".to_string()),
                discount_value: Some(Decimal::new(15, 0)),
                metadata: None,
                translations: vec![
                    BundleTranslationInput {
                        locale: "en".to_string(),
                        name: "Starter Kit".to_string(),
                        description: Some("Complete starter package".to_string()),
                    },
                    BundleTranslationInput {
                        locale: "ru".to_string(),
                        name: "Стартовый набор".to_string(),
                        description: Some("Полный комплект для новичка".to_string()),
                    },
                ],
                items: vec![
                    BundleItemInput {
                        product_id: product_id_1,
                        variant_id: None,
                        quantity: 1,
                        is_optional: Some(false),
                        discount_rate: None,
                        position: Some(0),
                    },
                    BundleItemInput {
                        product_id: product_id_2,
                        variant_id: None,
                        quantity: 2,
                        is_optional: Some(true),
                        discount_rate: Some(Decimal::new(10, 2)),
                        position: Some(1),
                    },
                ],
            },
        )
        .await
        .expect("Bundle creation failed");

    assert_eq!(created.slug, "starter-kit");
    assert_eq!(created.name, "Starter Kit");
    assert_eq!(created.bundle_type, "fixed");
    assert_eq!(created.translations.len(), 2);
    assert_eq!(created.items.len(), 2);

    // Query in Russian
    let fetched_ru = service
        .get_bundle(tenant_id, created.id, Some("ru"))
        .await
        .expect("Failed to fetch bundle in ru");
    assert_eq!(fetched_ru.name, "Стартовый набор");
    assert_eq!(
        fetched_ru.description.as_deref(),
        Some("Полный комплект для новичка")
    );

    // Query by slug
    let fetched_slug = service
        .get_bundle_by_slug(tenant_id, "starter-kit", Some("en"))
        .await
        .expect("Failed to fetch bundle by slug");
    assert_eq!(fetched_slug.name, "Starter Kit");
}

#[tokio::test]
async fn rejects_duplicate_slug_within_tenant() {
    let db = setup_test_db().await;
    let service = BundleService::new(db);
    let tenant_id = Uuid::new_v4();

    service
        .create_bundle(
            tenant_id,
            CreateBundleInput {
                bundle_product_id: None,
                slug: "summer-combo".to_string(),
                bundle_type: None,
                status: None,
                discount_type: None,
                discount_value: None,
                metadata: None,
                translations: vec![],
                items: vec![],
            },
        )
        .await
        .expect("First bundle creation should succeed");

    let err = service
        .create_bundle(
            tenant_id,
            CreateBundleInput {
                bundle_product_id: None,
                slug: "summer-combo".to_string(),
                bundle_type: None,
                status: None,
                discount_type: None,
                discount_value: None,
                metadata: None,
                translations: vec![],
                items: vec![],
            },
        )
        .await
        .expect_err("Duplicate slug must be rejected");

    match err {
        BundleError::SlugAlreadyExists(slug) => assert_eq!(slug, "summer-combo"),
        other => panic!("Expected SlugAlreadyExists, got: {:?}", other),
    }
}

#[tokio::test]
async fn updates_bundle_and_upserts_translations() {
    let db = setup_test_db().await;
    let service = BundleService::new(db);
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_bundle(
            tenant_id,
            CreateBundleInput {
                bundle_product_id: None,
                slug: "gift-set".to_string(),
                bundle_type: Some("fixed".to_string()),
                status: Some("draft".to_string()),
                discount_type: Some("none".to_string()),
                discount_value: None,
                metadata: None,
                translations: vec![BundleTranslationInput {
                    locale: "en".to_string(),
                    name: "Gift Set".to_string(),
                    description: None,
                }],
                items: vec![],
            },
        )
        .await
        .unwrap();

    let updated = service
        .update_bundle(
            tenant_id,
            created.id,
            UpdateBundleInput {
                status: Some("active".to_string()),
                discount_type: Some("fixed_amount".to_string()),
                discount_value: Some(Decimal::new(50, 0)),
                translations: Some(vec![
                    BundleTranslationInput {
                        locale: "en".to_string(),
                        name: "Holiday Gift Set".to_string(),
                        description: Some("Updated holiday edition".to_string()),
                    },
                    BundleTranslationInput {
                        locale: "ru".to_string(),
                        name: "Праздничный подарок".to_string(),
                        description: None,
                    },
                ]),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.status, "active");
    assert_eq!(updated.discount_type, "fixed_amount");
    assert_eq!(updated.translations.len(), 2);

    let en_trans = updated
        .translations
        .iter()
        .find(|t| t.locale == "en")
        .unwrap();
    assert_eq!(en_trans.name, "Holiday Gift Set");
}

#[tokio::test]
async fn adds_and_removes_bundle_items() {
    let db = setup_test_db().await;
    let service = BundleService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();

    insert_test_product(&db, tenant_id, product_id).await;

    let bundle = service
        .create_bundle(
            tenant_id,
            CreateBundleInput {
                bundle_product_id: None,
                slug: "kit-1".to_string(),
                bundle_type: Some("flexible".to_string()),
                status: Some("active".to_string()),
                discount_type: None,
                discount_value: None,
                metadata: None,
                translations: vec![],
                items: vec![],
            },
        )
        .await
        .unwrap();

    // Add item
    let added_item = service
        .add_bundle_item(
            tenant_id,
            bundle.id,
            BundleItemInput {
                product_id,
                variant_id: None,
                quantity: 3,
                is_optional: Some(true),
                discount_rate: None,
                position: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(added_item.product_id, product_id);
    assert_eq!(added_item.quantity, 3);
    assert!(added_item.is_optional);

    // Verify bundle has item
    let fetched = service
        .get_bundle(tenant_id, bundle.id, None)
        .await
        .unwrap();
    assert_eq!(fetched.items.len(), 1);

    // Remove item
    service
        .remove_bundle_item(tenant_id, bundle.id, added_item.id)
        .await
        .unwrap();

    let fetched_empty = service
        .get_bundle(tenant_id, bundle.id, None)
        .await
        .unwrap();
    assert_eq!(fetched_empty.items.len(), 0);
}

#[tokio::test]
async fn distinguishes_product_owned_bundles_from_component_membership() {
    let db = setup_test_db().await;
    let service = BundleService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    let owner_product = Uuid::new_v4();
    let component_product = Uuid::new_v4();

    insert_test_product(&db, tenant_id, owner_product).await;
    insert_test_product(&db, tenant_id, component_product).await;

    let owned_bundle = service
        .create_bundle(
            tenant_id,
            CreateBundleInput {
                bundle_product_id: Some(owner_product),
                slug: "owner-bundle".to_string(),
                bundle_type: Some("fixed".to_string()),
                status: Some("active".to_string()),
                discount_type: None,
                discount_value: None,
                metadata: None,
                translations: vec![],
                items: vec![BundleItemInput {
                    product_id: component_product,
                    variant_id: None,
                    quantity: 1,
                    is_optional: None,
                    discount_rate: None,
                    position: None,
                }],
            },
        )
        .await
        .unwrap();

    let owner_result = service
        .get_owned_bundle_for_product(tenant_id, owner_product, None)
        .await
        .unwrap()
        .expect("owner product should resolve its bundle");
    assert_eq!(owner_result.id, owned_bundle.id);

    assert!(
        service
            .get_owned_bundle_for_product(tenant_id, component_product, None)
            .await
            .unwrap()
            .is_none()
    );

    let owner_filtered_list = service
        .list_bundles(
            tenant_id,
            BundleFilter {
                search: None,
                status: None,
                bundle_type: None,
                owner_product_id: Some(owner_product),
            },
            1,
            10,
            None,
        )
        .await
        .unwrap();
    assert_eq!(owner_filtered_list.total, 1);
    assert_eq!(owner_filtered_list.items[0].id, owned_bundle.id);

    let duplicate_owner_error = service
        .create_bundle(
            tenant_id,
            CreateBundleInput {
                bundle_product_id: Some(owner_product),
                slug: "duplicate-owner-bundle".to_string(),
                bundle_type: None,
                status: None,
                discount_type: None,
                discount_value: None,
                metadata: None,
                translations: vec![],
                items: vec![],
            },
        )
        .await
        .expect_err("a Product may own only one bundle");
    assert!(matches!(
        duplicate_owner_error,
        BundleError::OwnerProductAlreadyHasBundle(product_id) if product_id == owner_product
    ));
}

#[tokio::test]
async fn rejects_invalid_bundle_values_before_writing() {
    let db = setup_test_db().await;
    let service = BundleService::new(db.clone());
    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();
    insert_test_product(&db, tenant_id, product_id).await;

    let invalid_discount = service
        .create_bundle(
            tenant_id,
            CreateBundleInput {
                bundle_product_id: None,
                slug: "invalid-discount".to_string(),
                bundle_type: Some("unknown".to_string()),
                status: None,
                discount_type: Some("percentage".to_string()),
                discount_value: Some(Decimal::new(101, 0)),
                metadata: None,
                translations: vec![],
                items: vec![],
            },
        )
        .await
        .expect_err("invalid bundle type must be rejected");
    assert!(matches!(invalid_discount, BundleError::InvalidInput(_)));

    let bundle = service
        .create_bundle(
            tenant_id,
            CreateBundleInput {
                bundle_product_id: None,
                slug: "valid-bundle".to_string(),
                bundle_type: None,
                status: None,
                discount_type: None,
                discount_value: None,
                metadata: Some(serde_json::json!({"source": "test"})),
                translations: vec![],
                items: vec![],
            },
        )
        .await
        .unwrap();

    let invalid_item = service
        .add_bundle_item(
            tenant_id,
            bundle.id,
            BundleItemInput {
                product_id,
                variant_id: None,
                quantity: 0,
                is_optional: None,
                discount_rate: None,
                position: None,
            },
        )
        .await
        .expect_err("zero quantity must be rejected");
    assert!(matches!(invalid_item, BundleError::InvalidInput(_)));
}
