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

    db.execute_unprepared(
        r#"
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
    UNIQUE (tenant_id, slug)
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
    bundle_id TEXT NOT NULL REFERENCES product_bundles(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    variant_id TEXT,
    quantity INTEGER NOT NULL DEFAULT 1,
    is_optional INTEGER NOT NULL DEFAULT 0,
    discount_rate REAL,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
"#,
    )
    .await
    .expect("Failed to create bundle tables");

    db
}

#[tokio::test]
async fn creates_and_queries_bundle_with_translations_and_items() {
    let db = setup_test_db().await;
    let service = BundleService::new(db);
    let tenant_id = Uuid::new_v4();
    let product_id_1 = Uuid::new_v4();
    let product_id_2 = Uuid::new_v4();

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

    let en_trans = updated.translations.iter().find(|t| t.locale == "en").unwrap();
    assert_eq!(en_trans.name, "Holiday Gift Set");
}

#[tokio::test]
async fn adds_and_removes_bundle_items() {
    let db = setup_test_db().await;
    let service = BundleService::new(db);
    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();

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
    let fetched = service.get_bundle(tenant_id, bundle.id, None).await.unwrap();
    assert_eq!(fetched.items.len(), 1);

    // Remove item
    service
        .remove_bundle_item(tenant_id, bundle.id, added_item.id)
        .await
        .unwrap();

    let fetched_empty = service.get_bundle(tenant_id, bundle.id, None).await.unwrap();
    assert_eq!(fetched_empty.items.len(), 0);
}

#[tokio::test]
async fn finds_bundles_for_product_and_filters_list() {
    let db = setup_test_db().await;
    let service = BundleService::new(db);
    let tenant_id = Uuid::new_v4();
    let product_a = Uuid::new_v4();
    let product_b = Uuid::new_v4();

    service
        .create_bundle(
            tenant_id,
            CreateBundleInput {
                bundle_product_id: None,
                slug: "bundle-alpha".to_string(),
                bundle_type: Some("fixed".to_string()),
                status: Some("active".to_string()),
                discount_type: None,
                discount_value: None,
                metadata: None,
                translations: vec![BundleTranslationInput {
                    locale: "en".to_string(),
                    name: "Alpha Bundle".to_string(),
                    description: None,
                }],
                items: vec![BundleItemInput {
                    product_id: product_a,
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

    service
        .create_bundle(
            tenant_id,
            CreateBundleInput {
                bundle_product_id: None,
                slug: "bundle-beta".to_string(),
                bundle_type: Some("flexible".to_string()),
                status: Some("draft".to_string()),
                discount_type: None,
                discount_value: None,
                metadata: None,
                translations: vec![BundleTranslationInput {
                    locale: "en".to_string(),
                    name: "Beta Bundle".to_string(),
                    description: None,
                }],
                items: vec![BundleItemInput {
                    product_id: product_b,
                    variant_id: None,
                    quantity: 2,
                    is_optional: None,
                    discount_rate: None,
                    position: None,
                }],
            },
        )
        .await
        .unwrap();

    // Query bundles for product_a
    let bundles_a = service
        .get_bundles_for_product(tenant_id, product_a, None)
        .await
        .unwrap();
    assert_eq!(bundles_a.len(), 1);
    assert_eq!(bundles_a[0].slug, "bundle-alpha");

    // Filter active only
    let active_list = service
        .list_bundles(
            tenant_id,
            BundleFilter {
                search: None,
                status: Some("active".to_string()),
                bundle_type: None,
            },
            1,
            10,
            None,
        )
        .await
        .unwrap();
    assert_eq!(active_list.total, 1);
    assert_eq!(active_list.items[0].slug, "bundle-alpha");
}
