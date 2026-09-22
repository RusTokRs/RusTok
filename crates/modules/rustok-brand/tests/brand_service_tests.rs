use rustok_brand::dto::{
    BrandFilter, BrandTranslationInput, CreateBrandInput, UpdateBrandInput,
};
use rustok_brand::error::BrandError;
use rustok_brand::ports::BrandPort;
use rustok_brand::services::BrandService;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use uuid::Uuid;

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory sqlite");

    db.execute_unprepared(
        r#"
CREATE TABLE brands (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    slug TEXT NOT NULL,
    logo_media_id TEXT,
    banner_media_id TEXT,
    website_url TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (tenant_id, slug)
);

CREATE TABLE brand_translations (
    id TEXT PRIMARY KEY,
    brand_id TEXT NOT NULL REFERENCES brands(id) ON DELETE CASCADE,
    locale TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (brand_id, locale)
);

CREATE TABLE brand_products (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    brand_id TEXT NOT NULL REFERENCES brands(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    is_primary INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    UNIQUE (tenant_id, brand_id, product_id)
);
"#,
    )
    .await
    .expect("Failed to create brand tables");

    db
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
    assert_eq!(fetched_ru.description.as_deref(), Some("Наковальни и ракеты"));

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
    assert_eq!(updated.website_url.as_deref(), Some("https://www.apple.com"));
    assert_eq!(updated.translations.len(), 2);

    let en_trans = updated.translations.iter().find(|t| t.locale == "en").unwrap();
    assert_eq!(en_trans.name, "Apple Inc.");

    let ru_trans = updated.translations.iter().find(|t| t.locale == "ru").unwrap();
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
}
