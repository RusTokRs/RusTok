use rustok_core::MigrationSource;
use rustok_product::dto::{
    CreateProductInput, CreateVariantInput, ProductBrandInput, ProductTranslationInput,
    UpdateProductInput,
};
use rustok_product::entities::{product, product_brand};
use rustok_product::migrations::m20261104_000039_create_product_brands;
use rustok_product::services::CatalogService;
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    EntityTrait, QueryFilter, Set,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn setup_database() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:product_brands_test_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    // Create prerequisite tables for SQLite tests
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS tenants (
            id TEXT NOT NULL PRIMARY KEY
        );
        CREATE TABLE IF NOT EXISTS users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            UNIQUE (tenant_id, id)
        );
        CREATE TABLE IF NOT EXISTS stock_locations (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            name TEXT,
            code TEXT,
            address_line1 TEXT,
            address_line2 TEXT,
            city TEXT,
            province TEXT,
            postal_code TEXT,
            country_code TEXT,
            phone TEXT,
            metadata TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            deleted_at TEXT
        );
        CREATE TABLE IF NOT EXISTS stock_location_translations (
            id TEXT NOT NULL PRIMARY KEY,
            stock_location_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            name TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS stock_location_translation_change_journal (
            change_seq INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            operation_id TEXT NOT NULL,
            tenant_id TEXT NOT NULL,
            stock_location_id TEXT NOT NULL,
            resource_revision TEXT NOT NULL,
            lifecycle TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS inventory_items (
            id TEXT NOT NULL PRIMARY KEY,
            variant_id TEXT NOT NULL,
            sku TEXT,
            requires_shipping INTEGER NOT NULL DEFAULT 1,
            metadata TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS inventory_levels (
            id TEXT PRIMARY KEY NOT NULL,
            inventory_item_id TEXT NOT NULL,
            location_id TEXT NOT NULL,
            stocked_quantity INTEGER NOT NULL DEFAULT 0,
            reserved_quantity INTEGER NOT NULL DEFAULT 0,
            incoming_quantity INTEGER NOT NULL DEFAULT 0,
            low_stock_threshold INTEGER,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS inventory_source_baselines (
            tenant_id TEXT NOT NULL PRIMARY KEY,
            baseline_id TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            active INTEGER NOT NULL DEFAULT 1,
            last_variant_id TEXT,
            record_count INTEGER NOT NULL DEFAULT 0,
            last_page_id TEXT,
            last_page_after TEXT,
            last_page_limit INTEGER,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS inventory_source_records (
            tenant_id TEXT NOT NULL,
            variant_id TEXT NOT NULL,
            baseline_id TEXT NOT NULL,
            inventory_item_id TEXT NOT NULL,
            product_id TEXT NOT NULL,
            source_version INTEGER NOT NULL DEFAULT 1,
            deleted INTEGER NOT NULL DEFAULT 0,
            stocked INTEGER NOT NULL DEFAULT 0,
            reserved INTEGER NOT NULL DEFAULT 0,
            incoming INTEGER NOT NULL DEFAULT 0,
            binding_digest TEXT NOT NULL DEFAULT '',
            last_change_id TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, variant_id)
        );
        CREATE TABLE IF NOT EXISTS inventory_source_changes (
            change_id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            variant_id TEXT NOT NULL,
            baseline_id TEXT NOT NULL,
            source_version INTEGER NOT NULL DEFAULT 1,
            kind TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS inventory_source_baseline_pages (
            tenant_id TEXT NOT NULL,
            page_id TEXT NOT NULL,
            baseline_id TEXT NOT NULL,
            after_variant_id TEXT,
            page_limit INTEGER NOT NULL,
            next_after TEXT,
            active INTEGER NOT NULL DEFAULT 1,
            record_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, page_id)
        );
        CREATE TABLE IF NOT EXISTS product_variant_translation_change_journal (
            change_seq INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            root_event_id TEXT NOT NULL UNIQUE,
            tenant_id TEXT NOT NULL,
            variant_id TEXT NOT NULL,
            resource_revision TEXT NOT NULL,
            lifecycle TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS prices (
            id TEXT NOT NULL PRIMARY KEY,
            variant_id TEXT NOT NULL,
            price_list_id TEXT,
            channel_id TEXT,
            channel_slug TEXT,
            currency_code TEXT NOT NULL,
            region_id TEXT,
            amount_decimal TEXT NOT NULL DEFAULT '0',
            compare_at_amount_decimal TEXT,
            amount INTEGER,
            compare_at_amount INTEGER,
            min_quantity INTEGER,
            max_quantity INTEGER
        );
        CREATE TABLE IF NOT EXISTS products (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'draft',
            seller_id TEXT,
            vendor TEXT,
            product_type TEXT,
            shipping_profile_slug TEXT,
            primary_category_id TEXT,
            metadata TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            published_at TEXT,
            revision INTEGER NOT NULL DEFAULT 1
        );
        CREATE TABLE IF NOT EXISTS product_translations (
            id TEXT PRIMARY KEY NOT NULL,
            product_id TEXT NOT NULL,
            tenant_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            title TEXT NOT NULL,
            handle TEXT NOT NULL,
            description TEXT,
            meta_title TEXT,
            meta_description TEXT,
            FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE,
            UNIQUE (tenant_id, locale, handle)
        );
        CREATE TABLE IF NOT EXISTS product_variants (
            id TEXT PRIMARY KEY NOT NULL,
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
            weight TEXT,
            weight_unit TEXT,
            combination_identity TEXT,
            position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS product_variant_translations (
            id TEXT PRIMARY KEY NOT NULL,
            variant_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            title TEXT,
            FOREIGN KEY (variant_id) REFERENCES product_variants(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS product_tags (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            product_id TEXT NOT NULL,
            term_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS product_variant_axes (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            product_id TEXT NOT NULL,
            attribute_id TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS product_variant_axis_values (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            axis_id TEXT NOT NULL,
            option_id TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS product_variant_attribute_values (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            variant_id TEXT NOT NULL,
            attribute_id TEXT NOT NULL,
            value_text TEXT,
            value_integer INTEGER,
            value_decimal NUMERIC,
            value_boolean BOOLEAN,
            value_date TEXT,
            value_datetime TEXT,
            value_json TEXT,
            detached_at TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS product_variant_attribute_value_translations (
            id TEXT PRIMARY KEY,
            value_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            value_text TEXT
        );
        CREATE TABLE IF NOT EXISTS product_variant_attribute_value_options (
            tenant_id TEXT NOT NULL,
            value_id TEXT NOT NULL,
            option_id TEXT NOT NULL,
            PRIMARY KEY (value_id, option_id)
        );
        CREATE TABLE IF NOT EXISTS product_images (
            id TEXT PRIMARY KEY NOT NULL,
            product_id TEXT NOT NULL,
            media_id TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS product_image_translations (
            id TEXT PRIMARY KEY NOT NULL,
            image_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            alt_text TEXT
        );
        CREATE TABLE IF NOT EXISTS product_attributes (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            code TEXT NOT NULL,
            value_type TEXT NOT NULL,
            scope TEXT NOT NULL DEFAULT 'product',
            is_localized BOOLEAN NOT NULL DEFAULT 0,
            is_filterable BOOLEAN NOT NULL DEFAULT 0,
            is_searchable BOOLEAN NOT NULL DEFAULT 0,
            is_sortable BOOLEAN NOT NULL DEFAULT 0,
            is_comparable BOOLEAN NOT NULL DEFAULT 0,
            show_on_storefront BOOLEAN NOT NULL DEFAULT 1,
            show_in_admin_grid BOOLEAN NOT NULL DEFAULT 0,
            search_weight INTEGER NOT NULL DEFAULT 1,
            filter_display TEXT,
            facet_mode TEXT,
            position INTEGER NOT NULL DEFAULT 0,
            validation TEXT NOT NULL DEFAULT '{}',
            default_value TEXT,
            metadata TEXT NOT NULL DEFAULT '{}',
            archived_at TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS product_attribute_values (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            product_id TEXT NOT NULL,
            attribute_id TEXT NOT NULL,
            value_text TEXT,
            value_integer INTEGER,
            value_decimal NUMERIC,
            value_boolean BOOLEAN,
            value_date TEXT,
            value_datetime TEXT,
            value_json TEXT,
            detached_at TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS product_attribute_options (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            attribute_id TEXT NOT NULL,
            code TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0,
            metadata TEXT NOT NULL DEFAULT '{}',
            archived_at TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS product_attribute_option_translations (
            id TEXT PRIMARY KEY,
            option_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            label TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS product_attribute_translations (
            id TEXT PRIMARY KEY,
            attribute_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            label TEXT NOT NULL,
            name TEXT,
            help_text TEXT,
            facet_label TEXT,
            seo_label TEXT
        );
        CREATE TABLE IF NOT EXISTS product_attribute_value_translations (
            id TEXT PRIMARY KEY,
            value_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            value_text TEXT
        );
        CREATE TABLE IF NOT EXISTS product_attribute_value_options (
            tenant_id TEXT NOT NULL,
            value_id TEXT NOT NULL,
            option_id TEXT NOT NULL,
            PRIMARY KEY (value_id, option_id)
        );
        CREATE TABLE IF NOT EXISTS catalog_categories (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            code TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'structural',
            path TEXT NOT NULL,
            level INTEGER NOT NULL DEFAULT 0,
            is_active INTEGER NOT NULL DEFAULT 1,
            rule_config TEXT NOT NULL DEFAULT '{}',
            metadata TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            deleted_at TEXT NULL
        );
        CREATE TABLE IF NOT EXISTS category_attribute_schema_assignments (
            tenant_id TEXT NOT NULL,
            category_id TEXT NOT NULL,
            schema_id TEXT NULL,
            mode TEXT NOT NULL DEFAULT 'inherit',
            snapshot TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, category_id)
        );
        CREATE TABLE IF NOT EXISTS category_attributes (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            category_id TEXT NOT NULL,
            attribute_id TEXT NOT NULL,
            group_id TEXT NULL,
            binding_kind TEXT NOT NULL DEFAULT 'direct',
            is_required INTEGER NULL,
            is_disabled INTEGER NOT NULL DEFAULT 0,
            position INTEGER NULL,
            visibility_overrides TEXT NOT NULL DEFAULT '{}',
            validation_overrides TEXT NOT NULL DEFAULT '{}',
            variant_axis_policy TEXT NOT NULL DEFAULT 'forbidden',
            default_variant_axis INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )
    .await?;

    let manager = SchemaManager::new(&db);
    for migration in rustok_outbox::OutboxModule.migrations() {
        migration.up(&manager).await?;
    }
    rustok_core::field_schema::create_field_definitions_table(&manager, "product", "products")
        .await?;
    // Exercise up migration
    m20261104_000039_create_product_brands::Migration
        .up(&manager)
        .await?;

    Ok(db)
}

#[tokio::test]
async fn product_brands_table_and_indexes_lifecycle() -> TestResult<()> {
    let db = setup_database().await?;
    let manager = SchemaManager::new(&db);

    // Test migration down and up idempotency
    m20261104_000039_create_product_brands::Migration
        .down(&manager)
        .await?;
    m20261104_000039_create_product_brands::Migration
        .up(&manager)
        .await?;

    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();
    let brand_id_1 = Uuid::new_v4();
    let brand_id_2 = Uuid::new_v4();
    let now = chrono::Utc::now();

    // Insert dummy product
    let prod = product::ActiveModel {
        id: Set(product_id),
        tenant_id: Set(tenant_id),
        status: Set(product::ProductStatus::Draft),
        seller_id: Set(None),
        vendor: Set(None),
        product_type: Set(None),
        shipping_profile_slug: Set(None),
        primary_category_id: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        published_at: Set(None),
        revision: Set(1),
    };
    prod.insert(&db).await?;

    // Insert primary brand
    let pb1 = product_brand::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        product_id: Set(product_id),
        brand_id: Set(brand_id_1),
        is_primary: Set(true),
        role: Set("manufacturer".to_string()),
        position: Set(0),
        created_at: Set(now.into()),
    };
    pb1.insert(&db).await?;

    // Insert collaborative brand
    let pb2 = product_brand::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        product_id: Set(product_id),
        brand_id: Set(brand_id_2),
        is_primary: Set(false),
        role: Set("collaboration".to_string()),
        position: Set(1),
        created_at: Set(now.into()),
    };
    pb2.insert(&db).await?;

    // Query back
    let brands = product_brand::Entity::find()
        .filter(product_brand::Column::ProductId.eq(product_id))
        .all(&db)
        .await?;
    assert_eq!(brands.len(), 2);

    // Invariant: attempting to insert a second primary brand violates uq_product_brands_primary
    let pb_conflict = product_brand::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        product_id: Set(product_id),
        brand_id: Set(Uuid::new_v4()),
        is_primary: Set(true),
        role: Set("partner".to_string()),
        position: Set(2),
        created_at: Set(now.into()),
    };
    let insert_result = pb_conflict.insert(&db).await;
    assert!(
        insert_result.is_err(),
        "Expected unique constraint error on multiple primary brands"
    );

    // Invariant: duplicate (tenant, product, brand) violates uq_product_brands
    let pb_duplicate = product_brand::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        product_id: Set(product_id),
        brand_id: Set(brand_id_1),
        is_primary: Set(false),
        role: Set("distributor".to_string()),
        position: Set(3),
        created_at: Set(now.into()),
    };
    let duplicate_result = pb_duplicate.insert(&db).await;
    assert!(
        duplicate_result.is_err(),
        "Expected unique constraint error on duplicate (tenant, product, brand)"
    );

    // Foreign key cascade: deleting product must delete product_brands
    product::Entity::delete_by_id(product_id).exec(&db).await?;
    let remaining_brands = product_brand::Entity::find()
        .filter(product_brand::Column::ProductId.eq(product_id))
        .all(&db)
        .await?;
    assert_eq!(
        remaining_brands.len(),
        0,
        "product_brands must cascade delete with product"
    );

    Ok(())
}

#[tokio::test]
async fn catalog_service_handles_product_brands_lifecycle() -> TestResult<()> {
    let db = setup_database().await?;
    let event_bus = mock_transactional_event_bus();
    let service = CatalogService::new(db.clone(), event_bus);

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let brand_id_primary = Uuid::new_v4();
    let brand_id_collab = Uuid::new_v4();

    // 1. Create product with single brand (omitting is_primary should default it to true)
    let input = CreateProductInput {
        translations: vec![ProductTranslationInput {
            locale: "en-US".to_string(),
            title: "Super Sneakers".to_string(),
            handle: Some("super-sneakers".to_string()),
            description: None,
            meta_title: None,
            meta_description: None,
        }],
        variants: vec![CreateVariantInput {
            sku: Some("SNEAK-01".to_string()),
            barcode: None,
            shipping_profile_slug: None,
            axis_values: vec![],
            prices: vec![],
            inventory_quantity: 0,
            inventory_policy: "deny".to_string(),
            weight: None,
            weight_unit: None,
        }],
        brands: vec![ProductBrandInput {
            brand_id: brand_id_primary,
            is_primary: false, // will auto-default to true because it is the sole brand
            role: "manufacturer".to_string(),
            position: 0,
        }],
        ..Default::default()
    };

    let created = service.create_product(tenant_id, actor_id, input).await?;
    assert_eq!(created.brands.len(), 1);
    assert_eq!(created.brands[0].brand_id, brand_id_primary);
    assert!(created.brands[0].is_primary);
    assert_eq!(created.brands[0].role, "manufacturer");

    // 2. Update product to include a co-branding partner
    let update_input = UpdateProductInput {
        expected_revision: Some(created.revision),
        brands: Some(vec![
            ProductBrandInput {
                brand_id: brand_id_primary,
                is_primary: true,
                role: "manufacturer".to_string(),
                position: 0,
            },
            ProductBrandInput {
                brand_id: brand_id_collab,
                is_primary: false,
                role: "collaboration".to_string(),
                position: 1,
            },
        ]),
        ..Default::default()
    };

    let updated = service
        .update_product(tenant_id, actor_id, created.id, update_input)
        .await?;
    assert_eq!(updated.brands.len(), 2);
    assert_eq!(updated.brands[0].brand_id, brand_id_primary);
    assert!(updated.brands[0].is_primary);
    assert_eq!(updated.brands[1].brand_id, brand_id_collab);
    assert!(!updated.brands[1].is_primary);
    assert_eq!(updated.brands[1].role, "collaboration");

    // 3. Validation: rejecting duplicate brand ID
    let duplicate_input = UpdateProductInput {
        expected_revision: Some(updated.revision),
        brands: Some(vec![
            ProductBrandInput {
                brand_id: brand_id_primary,
                is_primary: true,
                role: "manufacturer".to_string(),
                position: 0,
            },
            ProductBrandInput {
                brand_id: brand_id_primary,
                is_primary: false,
                role: "collaboration".to_string(),
                position: 1,
            },
        ]),
        ..Default::default()
    };
    let duplicate_result = service
        .update_product(tenant_id, actor_id, created.id, duplicate_input)
        .await;
    assert!(
        duplicate_result.is_err(),
        "Expected error on duplicate brand ID"
    );

    // 4. Validation: rejecting multiple primary brands
    let multi_primary_input = UpdateProductInput {
        expected_revision: Some(updated.revision),
        brands: Some(vec![
            ProductBrandInput {
                brand_id: brand_id_primary,
                is_primary: true,
                role: "manufacturer".to_string(),
                position: 0,
            },
            ProductBrandInput {
                brand_id: brand_id_collab,
                is_primary: true,
                role: "collaboration".to_string(),
                position: 1,
            },
        ]),
        ..Default::default()
    };
    let multi_primary_result = service
        .update_product(tenant_id, actor_id, created.id, multi_primary_input)
        .await;
    assert!(
        multi_primary_result.is_err(),
        "Expected error on multiple primary brands"
    );

    // 5. Validation: rejecting multiple brands without any primary brand
    let no_primary_input = UpdateProductInput {
        expected_revision: Some(updated.revision),
        brands: Some(vec![
            ProductBrandInput {
                brand_id: brand_id_primary,
                is_primary: false,
                role: "manufacturer".to_string(),
                position: 0,
            },
            ProductBrandInput {
                brand_id: brand_id_collab,
                is_primary: false,
                role: "collaboration".to_string(),
                position: 1,
            },
        ]),
        ..Default::default()
    };
    let no_primary_result = service
        .update_product(tenant_id, actor_id, created.id, no_primary_input)
        .await;
    assert!(
        no_primary_result.is_err(),
        "Expected error on multiple brands without primary"
    );

    // 6. Validation: rejecting nil UUID
    let nil_brand_input = UpdateProductInput {
        expected_revision: Some(updated.revision),
        brands: Some(vec![ProductBrandInput {
            brand_id: Uuid::nil(),
            is_primary: true,
            role: "manufacturer".to_string(),
            position: 0,
        }]),
        ..Default::default()
    };
    let nil_result = service
        .update_product(tenant_id, actor_id, created.id, nil_brand_input)
        .await;
    assert!(nil_result.is_err(), "Expected error on nil brand ID");

    Ok(())
}
