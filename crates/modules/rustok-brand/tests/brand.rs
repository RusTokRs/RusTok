use rustok_brand::dto::{BrandFilter, BrandTranslationInput, CreateBrandInput, UpdateBrandInput};
use rustok_brand::error::BrandError;
use rustok_brand::migrations::Migrator;
use rustok_brand::ports::BrandPort;
use rustok_brand::services::BrandService;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::{MigratorTrait, SchemaManager};
use uuid::Uuid;

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect to isolated in-memory SQLite");
    Migrator::up(&db, None)
        .await
        .expect("apply the shipped Brand migrations");
    db
}

async fn assert_migrated_owner_behavior(db: &DatabaseConnection) {
    let service = BrandService::new(db.clone());
    let tenant = Uuid::new_v4();
    let other_tenant = Uuid::new_v4();
    let translation = |locale: &str, name: &str| BrandTranslationInput {
        locale: locale.to_owned(),
        name: name.to_owned(),
        description: None,
    };
    let input = |translations| CreateBrandInput {
        slug: "acme".into(),
        logo_media_id: None,
        banner_media_id: None,
        website_url: None,
        is_active: None,
        metadata: None,
        translations,
    };

    // The second row conflicts after normalization. The earlier inserts in
    // this transaction must disappear as well, so the slug is reusable.
    assert!(
        service
            .create_brand(
                tenant,
                input(vec![
                    translation("en", "First"),
                    translation(" EN ", "Second")
                ])
            )
            .await
            .is_err()
    );
    assert!(matches!(
        service.get_brand_by_slug(tenant, "acme", None).await,
        Err(BrandError::SlugNotFound(_))
    ));

    let brand = service
        .create_brand(tenant, input(vec![translation("en", "Original")]))
        .await
        .expect("retry after rollback");
    let other_brand = service
        .create_brand(other_tenant, input(vec![translation("en", "Other tenant")]))
        .await
        .expect("slug is unique within a tenant");
    assert_ne!(brand.id, other_brand.id);
    assert!(matches!(
        service.get_brand(other_tenant, brand.id, Some("fr")).await,
        Err(BrandError::NotFound(id)) if id == brand.id
    ));

    let fallback = service
        .get_brand(tenant, brand.id, Some("fr"))
        .await
        .unwrap();
    assert_eq!(fallback.name, "Original");
    assert_eq!(fallback.translations.len(), 1);
    assert_eq!(fallback.translations[0].locale, "en");

    let updated = service
        .update_brand(
            tenant,
            brand.id,
            UpdateBrandInput {
                slug: None,
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: None,
                metadata: None,
                translations: Some(vec![translation("fr", "Version française")]),
            },
        )
        .await
        .expect("save explicit French content");
    assert_eq!(updated.translations.len(), 2);
    assert_eq!(
        service
            .get_brand(tenant, brand.id, Some("fr"))
            .await
            .unwrap()
            .name,
        "Version française"
    );
    assert_eq!(
        service
            .get_brand(tenant, brand.id, Some("en"))
            .await
            .unwrap()
            .name,
        "Original"
    );
}

async fn assert_migration_cycle(db: &DatabaseConnection) {
    assert_migrated_owner_behavior(db).await;
    // This destructive rollback is exercised only in an isolated test store.
    Migrator::down(db, None)
        .await
        .expect("roll back test migration");
    let schema = SchemaManager::new(db);
    for table in ["brands", "brand_translations", "brand_products"] {
        assert!(
            !schema.has_table(table).await.unwrap(),
            "{table} survived rollback"
        );
    }
    Migrator::up(db, None)
        .await
        .expect("reapply test migration");
    assert_migrated_owner_behavior(db).await;
}

#[tokio::test]
async fn sqlite_migration_and_owner_behavior() {
    let db = setup_test_db().await;
    assert_migration_cycle(&db).await;
}

#[tokio::test]
#[ignore = "requires RUSTOK_BRAND_TEST_POSTGRES_URL and permission to create a disposable schema"]
async fn postgres_migration_and_owner_behavior() {
    let url = std::env::var("RUSTOK_BRAND_TEST_POSTGRES_URL")
        .expect("set RUSTOK_BRAND_TEST_POSTGRES_URL to a PostgreSQL test database");
    assert!(url.starts_with("postgres://") || url.starts_with("postgresql://"));
    let control = Database::connect(&url)
        .await
        .expect("connect to test database");
    let schema = format!("brand_test_{}", Uuid::new_v4().simple());
    control
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();

    let mut options = ConnectOptions::new(url);
    options.set_schema_search_path(&schema);
    let result = tokio::spawn(async move {
        let db = Database::connect(options)
            .await
            .expect("connect to isolated schema");
        Migrator::up(&db, None)
            .await
            .expect("apply the shipped Brand migrations");
        assert_migration_cycle(&db).await;
        db.close().await.expect("close isolated connection pool");
    })
    .await;

    control
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error.into_panic());
    }
}

#[tokio::test]
async fn creates_and_queries_brand_with_translations() {
    let db = setup_test_db().await;
    let service = BrandService::new(db);
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "acme-corp".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: Some("https://acme.example.com".to_string()),
                is_active: Some(true),
                metadata: None,
                translations: vec![
                    BrandTranslationInput {
                        locale: "en".to_string(),
                        name: "ACME Corp".to_string(),
                        description: Some("Anvils and rockets".to_string()),
                    },
                    BrandTranslationInput {
                        locale: "ru".to_string(),
                        name: "АКМЕ Корп".to_string(),
                        description: Some("Наковальни и ракеты".to_string()),
                    },
                ],
            },
        )
        .await
        .expect("Brand creation failed");

    assert_eq!(created.slug, "acme-corp");
    assert_eq!(created.name, "ACME Corp");
    assert_eq!(created.translations.len(), 2);

    // Query by ID with locale "ru"
    let fetched_ru = service
        .get_brand(tenant_id, created.id, Some("ru"))
        .await
        .expect("Failed to fetch brand in ru");
    assert_eq!(fetched_ru.name, "АКМЕ Корп");
    assert_eq!(
        fetched_ru.description.as_deref(),
        Some("Наковальни и ракеты")
    );

    // Query by slug with locale "en"
    let fetched_slug = service
        .get_brand_by_slug(tenant_id, "acme-corp", Some("en"))
        .await
        .expect("Failed to fetch brand by slug");
    assert_eq!(fetched_slug.name, "ACME Corp");
}

#[tokio::test]
async fn rejects_duplicate_slug_within_tenant() {
    let db = setup_test_db().await;
    let service = BrandService::new(db);
    let tenant_id = Uuid::new_v4();

    service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "nike".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: None,
                metadata: None,
                translations: vec![BrandTranslationInput {
                    locale: "en".to_string(),
                    name: "Nike".to_string(),
                    description: None,
                }],
            },
        )
        .await
        .expect("First brand creation should succeed");

    let err = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "nike".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: None,
                metadata: None,
                translations: vec![BrandTranslationInput {
                    locale: "en".to_string(),
                    name: "Nike Alternate".to_string(),
                    description: None,
                }],
            },
        )
        .await
        .expect_err("Duplicate slug must be rejected");

    match err {
        BrandError::SlugAlreadyExists(slug) => assert_eq!(slug, "nike"),
        other => panic!("Expected SlugAlreadyExists, got: {:?}", other),
    }

    // Different tenant can use the same slug
    let other_tenant = Uuid::new_v4();
    let other_created = service
        .create_brand(
            other_tenant,
            CreateBrandInput {
                slug: "nike".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: None,
                metadata: None,
                translations: vec![],
            },
        )
        .await
        .expect("Different tenant should allow identical slug");

    assert_eq!(other_created.slug, "nike");
}

#[tokio::test]
async fn updates_brand_and_upserts_translations() {
    let db = setup_test_db().await;
    let service = BrandService::new(db);
    let tenant_id = Uuid::new_v4();

    let created = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "apple".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: Some("https://apple.com".to_string()),
                is_active: Some(true),
                metadata: None,
                translations: vec![BrandTranslationInput {
                    locale: "en".to_string(),
                    name: "Apple".to_string(),
                    description: Some("Think different".to_string()),
                }],
            },
        )
        .await
        .unwrap();

    let updated = service
        .update_brand(
            tenant_id,
            created.id,
            UpdateBrandInput {
                slug: Some("apple-inc".to_string()),
                website_url: Some(Some("https://www.apple.com".to_string())),
                translations: Some(vec![
                    // Update existing "en"
                    BrandTranslationInput {
                        locale: "en".to_string(),
                        name: "Apple Inc.".to_string(),
                        description: Some("Consumer electronics".to_string()),
                    },
                    // Add new "ru" translation
                    BrandTranslationInput {
                        locale: "ru".to_string(),
                        name: "Эппл".to_string(),
                        description: Some("Электроника".to_string()),
                    },
                ]),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.slug, "apple-inc");
    assert_eq!(
        updated.website_url.as_deref(),
        Some("https://www.apple.com")
    );
    assert_eq!(updated.translations.len(), 2);

    let en_trans = updated
        .translations
        .iter()
        .find(|t| t.locale == "en")
        .unwrap();
    assert_eq!(en_trans.name, "Apple Inc.");

    let ru_trans = updated
        .translations
        .iter()
        .find(|t| t.locale == "ru")
        .unwrap();
    assert_eq!(ru_trans.name, "Эппл");
}

#[tokio::test]
async fn assigns_and_retrieves_product_brand() {
    let db = setup_test_db().await;
    let service = BrandService::new(db);
    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();

    let brand = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "sony".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: Some(true),
                metadata: None,
                translations: vec![BrandTranslationInput {
                    locale: "en".to_string(),
                    name: "Sony".to_string(),
                    description: None,
                }],
            },
        )
        .await
        .unwrap();

    // No brand assigned yet
    let initial = service
        .get_brand_for_product(tenant_id, product_id, None)
        .await
        .unwrap();
    assert!(initial.is_none());

    // Assign brand to product
    service
        .assign_product_brand(tenant_id, brand.id, product_id, true)
        .await
        .unwrap();

    let assigned = service
        .get_brand_for_product(tenant_id, product_id, Some("en"))
        .await
        .unwrap()
        .expect("Product should now have a brand");

    assert_eq!(assigned.id, brand.id);
    assert_eq!(assigned.name, "Sony");

    // Unassign brand
    service
        .unassign_product_brand(tenant_id, brand.id, product_id)
        .await
        .unwrap();

    let unassigned = service
        .get_brand_for_product(tenant_id, product_id, None)
        .await
        .unwrap();
    assert!(unassigned.is_none());
}

#[tokio::test]
async fn lists_brands_with_filter() {
    let db = setup_test_db().await;
    let service = BrandService::new(db);
    let tenant_id = Uuid::new_v4();

    for slug in ["alpha", "beta", "gamma", "delta"] {
        service
            .create_brand(
                tenant_id,
                CreateBrandInput {
                    slug: slug.to_string(),
                    logo_media_id: None,
                    banner_media_id: None,
                    website_url: None,
                    is_active: Some(slug != "delta"),
                    metadata: None,
                    translations: vec![BrandTranslationInput {
                        locale: "en".to_string(),
                        name: slug.to_uppercase(),
                        description: None,
                    }],
                },
            )
            .await
            .unwrap();
    }

    // List all
    let all = service
        .list_brands(tenant_id, BrandFilter::default(), 1, 10, None)
        .await
        .unwrap();
    assert_eq!(all.total, 4);

    // List active only
    let active = service
        .list_brands(
            tenant_id,
            BrandFilter {
                search: None,
                is_active: Some(true),
            },
            1,
            10,
            None,
        )
        .await
        .unwrap();
    assert_eq!(active.total, 3);

    // List search "alp"
    let searched = service
        .list_brands(
            tenant_id,
            BrandFilter {
                search: Some("alp".to_string()),
                is_active: None,
            },
            1,
            10,
            None,
        )
        .await
        .unwrap();
    assert_eq!(searched.total, 1);
    assert_eq!(searched.items[0].slug, "alpha");

    let beyond_last_page = service
        .list_brands(tenant_id, BrandFilter::default(), u64::MAX, 100, None)
        .await
        .expect("an out-of-range page is empty");
    assert!(beyond_last_page.items.is_empty());
    assert_eq!(beyond_last_page.total, all.total);

    // A 100-byte cut would fall inside the first Cyrillic character.
    let unicode = format!("{}ж", "a".repeat(99));
    let no_match = service
        .list_brands(
            tenant_id,
            BrandFilter {
                search: Some(unicode),
                is_active: None,
            },
            1,
            10,
            None,
        )
        .await
        .expect("a long Unicode query remains a valid string");
    assert!(no_match.items.is_empty());
}

#[tokio::test]
async fn deletes_brand_with_cascading_associations() {
    let db = setup_test_db().await;
    let service = BrandService::new(db);
    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();

    let brand = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "cascading-test".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: Some(true),
                metadata: None,
                translations: vec![BrandTranslationInput {
                    locale: "en".to_string(),
                    name: "Cascading Test".to_string(),
                    description: None,
                }],
            },
        )
        .await
        .unwrap();

    // Assign to product
    service
        .assign_product_brand(tenant_id, brand.id, product_id, true)
        .await
        .unwrap();

    // Verify brand exists and is linked
    assert!(service.get_brand(tenant_id, brand.id, None).await.is_ok());
    assert!(
        service
            .get_brand_for_product(tenant_id, product_id, None)
            .await
            .unwrap()
            .is_some()
    );

    // Delete brand
    service
        .delete_brand(tenant_id, brand.id)
        .await
        .expect("Deletion should succeed");

    // Brand should not exist
    let get_err = service
        .get_brand(tenant_id, brand.id, None)
        .await
        .expect_err("Brand should be deleted");
    assert!(matches!(get_err, BrandError::NotFound(_)));

    // Product should no longer have brand
    let product_brand = service
        .get_brand_for_product(tenant_id, product_id, None)
        .await
        .unwrap();
    assert!(product_brand.is_none());
}

#[tokio::test]
async fn rejects_nil_uuids() {
    let db = setup_test_db().await;
    let service = BrandService::new(db);
    let valid_id = Uuid::new_v4();

    // Nil tenant_id in get_brand
    let err = service
        .get_brand(Uuid::nil(), valid_id, None)
        .await
        .unwrap_err();
    assert!(matches!(err, BrandError::InvalidInput(_)));

    // Nil brand_id in get_brand
    let err = service
        .get_brand(valid_id, Uuid::nil(), None)
        .await
        .unwrap_err();
    assert!(matches!(err, BrandError::InvalidInput(_)));

    // Nil tenant_id in create_brand
    let err = service
        .create_brand(
            Uuid::nil(),
            CreateBrandInput {
                slug: "valid".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: None,
                metadata: None,
                translations: vec![],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, BrandError::InvalidInput(_)));

    // Nil tenant_id in assign_product_brand
    let err = service
        .assign_product_brand(Uuid::nil(), valid_id, valid_id, true)
        .await
        .unwrap_err();
    assert!(matches!(err, BrandError::InvalidInput(_)));
}

#[tokio::test]
async fn rejects_invalid_bounds_and_lengths() {
    let db = setup_test_db().await;
    let service = BrandService::new(db);
    let tenant_id = Uuid::new_v4();

    // Empty slug
    let err = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "   ".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: None,
                metadata: None,
                translations: vec![],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, BrandError::InvalidInput(_)));

    // Overlong slug (> 100)
    let err = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "a".repeat(101),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: None,
                metadata: None,
                translations: vec![],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, BrandError::InvalidInput(_)));

    // Overlong website_url (> 512)
    let err = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "valid-slug".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: Some("https://".to_string() + &"a".repeat(506)),
                is_active: None,
                metadata: None,
                translations: vec![],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, BrandError::InvalidInput(_)));

    // Empty translation name
    let err = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "valid-slug-2".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: None,
                metadata: None,
                translations: vec![BrandTranslationInput {
                    locale: "en".to_string(),
                    name: "   ".to_string(),
                    description: None,
                }],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, BrandError::InvalidInput(_)));
}
