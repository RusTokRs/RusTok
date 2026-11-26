use rustok_brand::dto::{BrandFilter, BrandTranslationInput, CreateBrandInput, UpdateBrandInput};
use rustok_brand::error::BrandError;
use rustok_brand::migrations::Migrator;
use rustok_brand::ports::BrandPort;
use rustok_brand::services::BrandService;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
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

    // Duplicate locales after normalization are rejected before inserting rows,
    // keeping the error typed as InvalidInput and leaving the slug reusable.
    assert!(matches!(
        service
            .create_brand(
                tenant,
                input(vec![
                    translation("en", "First"),
                    translation(" EN ", "Second")
                ])
            )
            .await,
        Err(BrandError::InvalidInput(_))
    ));
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
    assert!(matches!(
        service.update_brand(other_tenant, brand.id, UpdateBrandInput {
            translations: Some(vec![translation("fr", "Wrong tenant")]),
            ..UpdateBrandInput::default()
        }).await,
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
                translations: Some(vec![translation("fr", "Version française")]),
                ..UpdateBrandInput::default()
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
    let schema = SchemaManager::new(db);
    assert!(
        !schema.has_table("brand_products").await.unwrap(),
        "brand_products should be dropped by contract migration"
    );
    assert_migrated_owner_behavior(db).await;
    // This destructive rollback is exercised only in an isolated test store.
    Migrator::down(db, None)
        .await
        .expect("roll back test migration");
    for table in [
        "brands",
        "brand_translations",
        "brand_products",
        "brand_categories",
    ] {
        assert!(
            !schema.has_table(table).await.unwrap(),
            "{table} survived rollback"
        );
    }
    Migrator::up(db, None)
        .await
        .expect("reapply test migration");
    assert!(
        !schema.has_table("brand_products").await.unwrap(),
        "brand_products should remain absent after reapplying migrations"
    );
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
    let expected_schema = schema.clone();
    let result = tokio::spawn(async move {
        let db = Database::connect(options)
            .await
            .expect("connect to isolated schema");
        let active_schema: String = db
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT current_schema()::text AS name",
            ))
            .await
            .expect("check test search path")
            .expect("current schema row")
            .try_get("", "name")
            .expect("current schema name");
        assert_eq!(active_schema, expected_schema);
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
                category_id: None,
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
                category_id: None,
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
                category_id: None,
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
async fn deletes_brand_with_cascading_category_associations() {
    use rustok_brand::ports::BrandReadPort;

    let db = setup_test_db().await;
    let service = BrandService::new(db);
    let tenant_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();

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

    // Assign to category
    service
        .assign_brand_category(tenant_id, brand.id, category_id, true)
        .await
        .unwrap();

    // Verify brand exists and is linked
    assert!(
        BrandPort::get_brand(&service, tenant_id, brand.id, None)
            .await
            .is_ok()
    );
    assert_eq!(
        service
            .list_brands_for_category(tenant_id, category_id, None)
            .await
            .unwrap()
            .len(),
        1
    );

    // Delete brand
    service
        .delete_brand(tenant_id, brand.id)
        .await
        .expect("Deletion should succeed");

    // Brand should not exist
    let get_err = BrandPort::get_brand(&service, tenant_id, brand.id, None)
        .await
        .expect_err("Brand should be deleted");
    assert!(matches!(get_err, BrandError::NotFound(_)));

    // Category should no longer list the deleted brand
    let category_brands = service
        .list_brands_for_category(tenant_id, category_id, None)
        .await
        .unwrap();
    assert!(category_brands.is_empty());
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

    // Nil tenant_id in assign_brand_category
    let err = service
        .assign_brand_category(Uuid::nil(), valid_id, valid_id, true)
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

#[tokio::test]
async fn assigns_and_filters_brand_categories() {
    use rustok_brand::ports::{BrandPort, BrandReadPort};

    let db = setup_test_db().await;
    let service = BrandService::new(db);
    let tenant_id = Uuid::new_v4();

    let clothing_cat = Uuid::new_v4();
    let electronics_cat = Uuid::new_v4();

    // Create 3 brands: Nike (clothing), Apple (electronics), Xiaomi (both)
    let nike = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "nike".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: Some(true),
                metadata: None,
                translations: vec![BrandTranslationInput {
                    locale: "en".to_string(),
                    name: "Nike".to_string(),
                    description: Some("Sportswear brand".to_string()),
                }],
            },
        )
        .await
        .unwrap();

    let apple = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "apple".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: Some(true),
                metadata: None,
                translations: vec![BrandTranslationInput {
                    locale: "en".to_string(),
                    name: "Apple".to_string(),
                    description: Some("Electronics brand".to_string()),
                }],
            },
        )
        .await
        .unwrap();

    let xiaomi = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "xiaomi".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: Some(true),
                metadata: None,
                translations: vec![BrandTranslationInput {
                    locale: "en".to_string(),
                    name: "Xiaomi".to_string(),
                    description: Some("Tech & lifestyle".to_string()),
                }],
            },
        )
        .await
        .unwrap();

    // Assign categories
    service
        .assign_brand_category(tenant_id, nike.id, clothing_cat, true)
        .await
        .unwrap();
    service
        .assign_brand_category(tenant_id, apple.id, electronics_cat, true)
        .await
        .unwrap();
    service
        .assign_brand_category(tenant_id, xiaomi.id, clothing_cat, false)
        .await
        .unwrap();
    service
        .assign_brand_category(tenant_id, xiaomi.id, electronics_cat, true)
        .await
        .unwrap();

    // Verify categories for brand
    let nike_cats = service
        .list_categories_for_brand(tenant_id, nike.id)
        .await
        .unwrap();
    assert_eq!(nike_cats.len(), 1);
    assert_eq!(nike_cats[0].category_id, clothing_cat);
    assert!(nike_cats[0].is_featured);

    let xiaomi_cats = service
        .list_categories_for_brand(tenant_id, xiaomi.id)
        .await
        .unwrap();
    assert_eq!(xiaomi_cats.len(), 2);

    // Filter brands by category (BrandFilter)
    let clothing_brands = service
        .list_brands(
            tenant_id,
            BrandFilter {
                category_id: Some(clothing_cat),
                ..Default::default()
            },
            1,
            20,
            None,
        )
        .await
        .unwrap();
    assert_eq!(clothing_brands.total, 2);
    let slugs: Vec<_> = clothing_brands
        .items
        .iter()
        .map(|b| b.slug.as_str())
        .collect();
    assert!(slugs.contains(&"nike"));
    assert!(slugs.contains(&"xiaomi"));
    assert!(!slugs.contains(&"apple"));

    let electronics_brands = service
        .list_brands(
            tenant_id,
            BrandFilter {
                category_id: Some(electronics_cat),
                ..Default::default()
            },
            1,
            20,
            None,
        )
        .await
        .unwrap();
    assert_eq!(electronics_brands.total, 2);
    let elec_slugs: Vec<_> = electronics_brands
        .items
        .iter()
        .map(|b| b.slug.as_str())
        .collect();
    assert!(elec_slugs.contains(&"apple"));
    assert!(elec_slugs.contains(&"xiaomi"));
    assert!(!elec_slugs.contains(&"nike"));

    // Test BrandReadPort: validate_brand_ids
    let non_existent = Uuid::new_v4();
    let validated = service
        .validate_brand_ids(tenant_id, &[nike.id, non_existent, apple.id])
        .await
        .unwrap();
    assert_eq!(validated.len(), 2);
    assert!(validated.contains(&nike.id));
    assert!(validated.contains(&apple.id));
    assert!(!validated.contains(&non_existent));

    // Test BrandReadPort: list_brands_for_category
    let cat_brands = service
        .list_brands_for_category(tenant_id, clothing_cat, None)
        .await
        .unwrap();
    assert_eq!(cat_brands.len(), 2);
    // Nike is featured, so it comes first
    assert_eq!(cat_brands[0].id, nike.id);
    assert_eq!(cat_brands[1].id, xiaomi.id);

    // Unassign category
    service
        .unassign_brand_category(tenant_id, xiaomi.id, clothing_cat)
        .await
        .unwrap();
    let updated_clothing_brands = service
        .list_brands_for_category(tenant_id, clothing_cat, None)
        .await
        .unwrap();
    assert_eq!(updated_clothing_brands.len(), 1);
    assert_eq!(updated_clothing_brands[0].id, nike.id);
}

#[tokio::test]
async fn drop_brand_products_migration_backfills_into_product_brands_safely() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect to isolated in-memory SQLite");

    db.execute_unprepared(
        r#"
CREATE TABLE products (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL
);

CREATE TABLE product_brands (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    product_id TEXT NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    brand_id TEXT NOT NULL,
    is_primary INTEGER NOT NULL DEFAULT 0,
    role TEXT NOT NULL DEFAULT 'manufacturer',
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX uq_product_brands ON product_brands (tenant_id, product_id, brand_id);
CREATE UNIQUE INDEX uq_product_brands_primary ON product_brands (tenant_id, product_id) WHERE is_primary = 1;
"#,
    )
    .await
    .expect("create product_brands host schema");

    // Apply only the first two Brand migrations so brand_products still exists.
    Migrator::up(&db, Some(2))
        .await
        .expect("apply initial Brand migrations");

    let tenant_id = Uuid::new_v4();
    let service = BrandService::new(db.clone());
    let brand_a = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "brand-a".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: Some(true),
                metadata: None,
                translations: vec![BrandTranslationInput {
                    locale: "en".to_string(),
                    name: "Brand A".to_string(),
                    description: None,
                }],
            },
        )
        .await
        .unwrap();
    let brand_b = service
        .create_brand(
            tenant_id,
            CreateBrandInput {
                slug: "brand-b".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: Some(true),
                metadata: None,
                translations: vec![BrandTranslationInput {
                    locale: "en".to_string(),
                    name: "Brand B".to_string(),
                    description: None,
                }],
            },
        )
        .await
        .unwrap();

    let product_with_dups = Uuid::new_v4();
    let product_with_existing_primary = Uuid::new_v4();
    let product_with_non_primary = Uuid::new_v4();
    let orphan_product = Uuid::new_v4();
    let other_tenant = Uuid::new_v4();
    let other_brand = service
        .create_brand(
            other_tenant,
            CreateBrandInput {
                slug: "other-brand".to_string(),
                logo_media_id: None,
                banner_media_id: None,
                website_url: None,
                is_active: Some(true),
                metadata: None,
                translations: vec![BrandTranslationInput {
                    locale: "en".to_string(),
                    name: "Other Brand".to_string(),
                    description: None,
                }],
            },
        )
        .await
        .unwrap();

    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO products (id, tenant_id) VALUES (?1, ?2), (?3, ?4), (?5, ?6)",
        vec![
            product_with_dups.into(),
            tenant_id.into(),
            product_with_existing_primary.into(),
            tenant_id.into(),
            product_with_non_primary.into(),
            tenant_id.into(),
        ],
    ))
    .await
    .unwrap();

    // Product already has brand_a as primary in product_brands.
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO product_brands (id, tenant_id, product_id, brand_id, is_primary, role, position, created_at) VALUES (?1, ?2, ?3, ?4, 1, 'manufacturer', 0, '2026-11-04T00:00:00Z')",
        vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            product_with_existing_primary.into(),
            brand_a.id.into(),
        ],
    ))
    .await
    .unwrap();

    // Legacy brand_products rows:
    // 1) Two primary rows for product_with_dups (second must downgrade to non-primary)
    // 2) Already-migrated (product_with_existing_primary, brand_a) + new primary-flagged brand_b
    // 3) Single non-primary row for product_with_non_primary (must promote to primary)
    // 4) Cross-tenant brand_id mismatch (must be ignored safely)
    // 5) Orphan product row with no matching products row (must be ignored safely)
    for (brand_id, product_id, is_primary, created_at) in [
        (brand_a.id, product_with_dups, 1, "2026-10-01T00:00:00Z"),
        (brand_b.id, product_with_dups, 1, "2026-10-02T00:00:00Z"),
        (
            brand_a.id,
            product_with_existing_primary,
            1,
            "2026-10-01T00:00:00Z",
        ),
        (
            brand_b.id,
            product_with_existing_primary,
            1,
            "2026-10-02T00:00:00Z",
        ),
        (
            brand_a.id,
            product_with_non_primary,
            0,
            "2026-10-01T00:00:00Z",
        ),
        (
            other_brand.id,
            product_with_non_primary,
            0,
            "2026-10-02T00:00:00Z",
        ),
        (brand_a.id, orphan_product, 1, "2026-10-03T00:00:00Z"),
    ] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO brand_products (id, tenant_id, brand_id, product_id, is_primary, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            vec![
                Uuid::new_v4().into(),
                tenant_id.into(),
                brand_id.into(),
                product_id.into(),
                is_primary.into(),
                created_at.into(),
            ],
        ))
        .await
        .unwrap();
    }

    // Apply the Phase 4 contract migration.
    Migrator::up(&db, None)
        .await
        .expect("contract migration should backfill and drop brand_products");

    assert!(
        !SchemaManager::new(&db)
            .has_table("brand_products")
            .await
            .unwrap()
    );

    let rows = db
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT product_id, brand_id, is_primary, role FROM product_brands ORDER BY product_id, is_primary DESC, brand_id",
        ))
        .await
        .unwrap();
    assert_eq!(rows.len(), 5);

    let dups_rows: Vec<(Uuid, i64, String)> = rows
        .iter()
        .filter(|r| r.try_get::<Uuid>("", "product_id").unwrap() == product_with_dups)
        .map(|r| {
            (
                r.try_get("", "brand_id").unwrap(),
                r.try_get("", "is_primary").unwrap(),
                r.try_get("", "role").unwrap(),
            )
        })
        .collect();
    assert_eq!(
        dups_rows,
        vec![
            (brand_a.id, 1, "manufacturer".to_string()),
            (brand_b.id, 0, "collaboration".to_string()),
        ]
    );

    let existing_rows: Vec<(Uuid, i64, String)> = rows
        .iter()
        .filter(|r| r.try_get::<Uuid>("", "product_id").unwrap() == product_with_existing_primary)
        .map(|r| {
            (
                r.try_get("", "brand_id").unwrap(),
                r.try_get("", "is_primary").unwrap(),
                r.try_get("", "role").unwrap(),
            )
        })
        .collect();
    assert_eq!(
        existing_rows,
        vec![
            (brand_a.id, 1, "manufacturer".to_string()),
            (brand_b.id, 0, "collaboration".to_string()),
        ]
    );

    let non_primary_rows: Vec<(Uuid, i64, String)> = rows
        .iter()
        .filter(|r| r.try_get::<Uuid>("", "product_id").unwrap() == product_with_non_primary)
        .map(|r| {
            (
                r.try_get("", "brand_id").unwrap(),
                r.try_get("", "is_primary").unwrap(),
                r.try_get("", "role").unwrap(),
            )
        })
        .collect();
    assert_eq!(
        non_primary_rows,
        vec![(brand_a.id, 1, "manufacturer".to_string())]
    );
}
