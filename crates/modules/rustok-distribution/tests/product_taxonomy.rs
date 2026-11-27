#![cfg(feature = "mod-product")]

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_distribution::in_process_product_taxonomy_port;
use rustok_outbox::SysEventsMigration;
use rustok_product::services::{
    CatalogCategoryKind, CategoryTranslationInput, CreateCatalogCategoryInput,
    ProductCatalogSchemaService,
};
use rustok_product::{CommerceError, ports::ProductTaxonomyCategoryTranslationInput};
use rustok_taxonomy::{
    SyncModuleCategoryInput, TaxonomyModule, TaxonomyOwnerCategoryReader, TaxonomyScopeType,
    TaxonomyService, TaxonomyTermKind, sync_module_category_in_tx,
};
use rustok_test_utils::db::setup_test_db;
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, FromQueryResult, Statement,
    TransactionTrait,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(FromQueryResult)]
struct TableInfoRow {
    name: String,
}

#[derive(FromQueryResult)]
struct CountRow {
    count: i64,
}

async fn setup_database() -> TestResult<DatabaseConnection> {
    let url = format!(
        "sqlite:file:product_category_taxonomy_same_id_cutover_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute_unprepared(
        "CREATE TABLE tenants (\
            id TEXT NOT NULL PRIMARY KEY\
        ); \
        CREATE TABLE users (\
            id TEXT NOT NULL PRIMARY KEY, \
            tenant_id TEXT NOT NULL, \
            UNIQUE (tenant_id, id)\
        );",
    )
    .await?;

    let manager = SchemaManager::new(&db);
    SysEventsMigration.up(&manager).await?;
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }

    // Canonical target schema for Product category storage without legacy/mirror columns or bindings.
    db.execute_unprepared(
        r#"
        CREATE TABLE catalog_categories (
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

        CREATE TABLE catalog_category_seo_translations (
            tenant_id TEXT NOT NULL,
            category_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            meta_title TEXT NULL,
            meta_description TEXT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, category_id, locale)
        );

        CREATE TABLE category_attribute_schema_assignments (
            tenant_id TEXT NOT NULL,
            category_id TEXT NOT NULL,
            schema_id TEXT NULL,
            mode TEXT NOT NULL DEFAULT 'inherit',
            snapshot TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, category_id)
        );

        CREATE TABLE category_attribute_groups (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            category_id TEXT NOT NULL,
            code TEXT NOT NULL,
            inherited_from_group_id TEXT NULL,
            position INTEGER NOT NULL DEFAULT 0,
            metadata TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE category_attribute_group_translations (
            id TEXT PRIMARY KEY NOT NULL,
            group_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            label TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE category_attributes (
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
        );

        CREATE TABLE product_attribute_schemas (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            code TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            archived_at TEXT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE product_attribute_schema_attributes (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            attribute_id TEXT NOT NULL,
            group_id TEXT NULL,
            is_required INTEGER NULL,
            is_disabled INTEGER NOT NULL DEFAULT 0,
            position INTEGER NULL,
            visibility_overrides TEXT NOT NULL DEFAULT '{}',
            validation_overrides TEXT NOT NULL DEFAULT '{}',
            variant_axis_policy TEXT NOT NULL DEFAULT 'forbidden',
            default_variant_axis INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE product_attribute_schema_groups (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            schema_id TEXT NOT NULL,
            code TEXT NOT NULL,
            position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE product_attribute_schema_group_translations (
            id TEXT PRIMARY KEY NOT NULL,
            group_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            label TEXT NOT NULL
        );
        "#,
    )
    .await?;

    Ok(db)
}

#[tokio::test]
async fn product_category_schema_and_service_use_canonical_same_id_taxonomy() -> TestResult<()> {
    let db = setup_database().await?;
    let tenant_id = Uuid::new_v4();

    // 1. Verify that the transitional binding table does NOT exist in the database.
    let binding_table_count = CountRow::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'product_catalog_category_taxonomy_bindings'",
        vec![],
    ))
    .one(&db)
    .await?
    .expect("count row")
    .count;
    assert_eq!(
        binding_table_count, 0,
        "product_catalog_category_taxonomy_bindings table must be completely dropped"
    );

    // 2. Verify that catalog_categories columns omit parent_id, slug, and position.
    let columns = TableInfoRow::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        "SELECT name FROM pragma_table_info('catalog_categories')",
        vec![],
    ))
    .all(&db)
    .await?
    .into_iter()
    .map(|r| r.name)
    .collect::<Vec<_>>();

    assert!(columns.contains(&"id".to_string()));
    assert!(columns.contains(&"tenant_id".to_string()));
    assert!(columns.contains(&"code".to_string()));
    assert!(columns.contains(&"kind".to_string()));
    assert!(columns.contains(&"path".to_string()));
    assert!(columns.contains(&"level".to_string()));
    assert!(
        !columns.contains(&"parent_id".to_string()),
        "parent_id must be removed from catalog_categories"
    );
    assert!(
        !columns.contains(&"slug".to_string()),
        "slug must be removed from catalog_categories"
    );
    assert!(
        !columns.contains(&"position".to_string()),
        "position must be removed from catalog_categories"
    );

    // 3. Create parent and child categories using ProductCatalogSchemaService.
    let event_bus = mock_transactional_event_bus();
    let service = ProductCatalogSchemaService::new(db.clone(), event_bus)
        .with_taxonomy_port(in_process_product_taxonomy_port(db.clone()));
    let actor_id = Uuid::new_v4();

    let parent_input = CreateCatalogCategoryInput {
        code: "apparel".to_string(),
        slug: "apparel".to_string(),
        kind: CatalogCategoryKind::Structural,
        parent_id: None,
        position: 0,
        rule_config: serde_json::json!({}),
        metadata: serde_json::json!({}),
        translations: vec![CategoryTranslationInput {
            locale: "en".to_string(),
            name: "Apparel".to_string(),
            description: Some("Apparel root category".to_string()),
            meta_title: Some("Apparel SEO".to_string()),
            meta_description: Some("Apparel SEO description".to_string()),
        }],
    };

    let parent = service
        .create_category(tenant_id, actor_id, parent_input)
        .await?;

    let child_input = CreateCatalogCategoryInput {
        code: "shirts".to_string(),
        slug: "shirts".to_string(),
        kind: CatalogCategoryKind::Structural,
        parent_id: Some(parent.id),
        position: 0,
        rule_config: serde_json::json!({}),
        metadata: serde_json::json!({}),
        translations: vec![CategoryTranslationInput {
            locale: "en".to_string(),
            name: "Shirts".to_string(),
            description: Some("Shirts subcategory".to_string()),
            meta_title: None,
            meta_description: None,
        }],
    };

    let child = service
        .create_category(tenant_id, actor_id, child_input)
        .await?;

    // 4. Assert Same-ID: taxonomy_terms contains entries where id matches category id directly.
    let parent_term_count = CountRow::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        "SELECT COUNT(*) AS count FROM taxonomy_terms WHERE id = $1 AND tenant_id = $2 AND scope_type = 'module' AND scope_value = 'product'",
        vec![parent.id.into(), tenant_id.into()],
    ))
    .one(&db)
    .await?
    .expect("count row")
    .count;
    assert_eq!(
        parent_term_count, 1,
        "parent category must have direct Same-ID in taxonomy_terms"
    );

    let child_term_count = CountRow::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        "SELECT COUNT(*) AS count FROM taxonomy_terms WHERE id = $1 AND tenant_id = $2 AND scope_type = 'module' AND scope_value = 'product'",
        vec![child.id.into(), tenant_id.into()],
    ))
    .one(&db)
    .await?
    .expect("count row")
    .count;
    assert_eq!(
        child_term_count, 1,
        "child category must have direct Same-ID in taxonomy_terms"
    );

    // 5. Verify list_categories reads from Taxonomy projections using Same-ID.
    let categories = service.list_categories(tenant_id, "en").await?;
    assert_eq!(categories.len(), 2);

    let listed_parent = categories
        .iter()
        .find(|c| c.id == parent.id)
        .expect("parent found");
    assert_eq!(listed_parent.name, "Apparel");
    assert_eq!(listed_parent.slug, "apparel");
    assert_eq!(listed_parent.parent_id, None);

    let listed_child = categories
        .iter()
        .find(|c| c.id == child.id)
        .expect("child found");
    assert_eq!(listed_child.name, "Shirts");
    assert_eq!(listed_child.slug, "shirts");
    assert_eq!(listed_child.parent_id, Some(parent.id));

    // 6. Verify effective forms load hierarchy from Taxonomy owner parent map.
    let labels = service
        .load_effective_form_group_labels(tenant_id, child.id, "en")
        .await?;
    assert!(labels.is_empty()); // Ancestor chain traversed without error

    let effective_form = service
        .load_effective_form_for_category(tenant_id, child.id, &[])
        .await?;
    assert_eq!(effective_form.category_id, child.id);

    // Missing placement is corrupt owner state, not a root category. Check
    // both connection variants, including an uncommitted deletion.
    let port = in_process_product_taxonomy_port(db.clone());
    let txn = db.begin().await?;
    txn.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "DELETE FROM taxonomy_category_hierarchy WHERE tenant_id = $1 AND term_id = $2",
        vec![tenant_id.into(), child.id.into()],
    ))
    .await?;
    let selected = [child.id];
    let in_tx = port
        .load_scoped_categories((&txn).into(), tenant_id, Some(&selected), "en", None)
        .await;
    assert!(matches!(
        in_tx,
        Err(CommerceError::Core(rustok_core::error::Error::Internal(_)))
    ));
    txn.commit().await?;
    let from_pool = service.list_categories(tenant_id, "en").await;
    assert!(matches!(
        from_pool,
        Err(CommerceError::Core(rustok_core::error::Error::Internal(_)))
    ));

    Ok(())
}

#[tokio::test]
async fn product_category_copy_preserves_taxonomy_presentation_and_routes() {
    let db = setup_test_db().await;
    let manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await.unwrap();
    }
    let tenant_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let canonical_key = format!("product-category-{category_id}");
    let initial = SyncModuleCategoryInput {
        category_id,
        module_scope: "product".into(),
        canonical_key: canonical_key.clone(),
        locale: "en".into(),
        name: "Shoes".into(),
        slug: "shoes".into(),
        aliases: vec!["boots".into()],
        description: Some("Shoes description".into()),
        parent_id: None,
        position: 0,
        icon_key: Some("headphones".into()),
        color: Some("#123456".into()),
    };
    let txn = db.begin().await.unwrap();
    sync_module_category_in_tx(&txn, tenant_id, initial)
        .await
        .unwrap();
    txn.commit().await.unwrap();

    let service = TaxonomyService::new(db.clone());
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let before = service
        .get_category_presentation(tenant_id, security.clone(), category_id)
        .await
        .unwrap();
    let port = in_process_product_taxonomy_port(db.clone());
    let input = ProductTaxonomyCategoryTranslationInput {
        category_id,
        module_scope: "product".into(),
        canonical_key,
        locale: "en".into(),
        name: "Footwear".into(),
        slug: "footwear".into(),
        aliases: vec![],
        description: None,
        parent_id: None,
        position: 0,
    };
    let txn = db.begin().await.unwrap();
    port.sync_category_translation_in_tx(&txn, tenant_id, input.clone())
        .await
        .unwrap();
    txn.commit().await.unwrap();

    let txn = db.begin().await.unwrap();
    let reclaim = port
        .sync_category_translation_in_tx(
            &txn,
            tenant_id,
            ProductTaxonomyCategoryTranslationInput {
                slug: "shoes".into(),
                ..input
            },
        )
        .await;
    assert!(matches!(reclaim, Err(CommerceError::Conflict(_))));
    txn.rollback().await.unwrap();

    let after = service
        .get_category_presentation(tenant_id, security, category_id)
        .await
        .unwrap();
    assert_eq!(after, before);
    for (slug, is_alias) in [("boots", true), ("footwear", false), ("shoes", true)] {
        let route = service
            .resolve_term_route_for_module(
                tenant_id,
                TaxonomyTermKind::Category,
                "product",
                "en",
                None,
                slug,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(route.term_id, category_id);
        assert_eq!(route.alias_id.is_some(), is_alias);
    }
    let categories = TaxonomyOwnerCategoryReader::new(db)
        .load_scoped_categories(
            tenant_id,
            TaxonomyScopeType::Module,
            Some("product"),
            Some(&[category_id]),
            "en",
            None,
        )
        .await
        .unwrap();
    assert_eq!(categories.len(), 1);
    assert_eq!(categories[0].slug, "footwear");
    assert_eq!(categories[0].name, "Footwear");
}
