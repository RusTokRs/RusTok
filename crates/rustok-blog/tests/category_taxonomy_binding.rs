use rustok_blog::{
    BlogCategoryTaxonomyBindingService, BlogModule, entities::blog_category_taxonomy_binding,
};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyModule, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, EntityTrait,
    Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const BLOG_BINDING_SCHEMA_MIGRATIONS: &[&str] = &[
    "m20260328_000001_create_blog_post_tables",
    "m20260328_000002_create_blog_taxonomy_tables",
    "m20260803_000016_add_blog_category_translation_target_support",
    "m20260812_000017_enforce_blog_category_hierarchy",
    "m20260824_000019_add_blog_taxonomy_category_binding",
];

async fn setup() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:blog_category_taxonomy_binding_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(opts)
        .await
        .expect("Blog Taxonomy binding database should connect");
    let schema = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("taxonomy migration should apply");
    }

    let mut applied_blog_migrations = 0usize;
    for migration in BlogModule.migrations() {
        if BLOG_BINDING_SCHEMA_MIGRATIONS.contains(&migration.name()) {
            migration
                .up(&schema)
                .await
                .expect("required Blog binding migration should apply");
            applied_blog_migrations += 1;
        }
    }
    assert_eq!(
        applied_blog_migrations,
        BLOG_BINDING_SCHEMA_MIGRATIONS.len(),
        "every required Blog binding migration must be registered"
    );
    db
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn create_blog_category(db: &DatabaseConnection, tenant_id: Uuid) -> Uuid {
    let category_id = Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO blog_categories (id, tenant_id) VALUES (?, ?)",
        [category_id.into(), tenant_id.into()],
    ))
    .await
    .expect("Blog category owner row should be created");
    category_id
}

async fn create_taxonomy_term(
    service: &TaxonomyService,
    tenant_id: Uuid,
    kind: TaxonomyTermKind,
    name: &str,
) -> Uuid {
    service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: name.to_string(),
                slug: None,
                canonical_key: Some(format!("{}-{}", name.to_ascii_lowercase(), Uuid::new_v4())),
                description: None,
                aliases: Vec::new(),
            },
        )
        .await
        .expect("taxonomy term should be created")
}

#[tokio::test]
async fn blog_category_binding_is_category_only_tenant_bounded_and_one_to_one() {
    let db = setup().await;
    let taxonomy = TaxonomyService::new(db.clone());
    let binding = BlogCategoryTaxonomyBindingService::new(db.clone());

    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let blog_category_id = create_blog_category(&db, tenant_id).await;
    let second_blog_category_id = create_blog_category(&db, tenant_id).await;

    let taxonomy_category_id = create_taxonomy_term(
        &taxonomy,
        tenant_id,
        TaxonomyTermKind::Category,
        "Blog Taxonomy Category",
    )
    .await;
    let second_taxonomy_category_id = create_taxonomy_term(
        &taxonomy,
        tenant_id,
        TaxonomyTermKind::Category,
        "Blog Secondary Category",
    )
    .await;
    let tag_id = create_taxonomy_term(
        &taxonomy,
        tenant_id,
        TaxonomyTermKind::Tag,
        "Blog binding tag",
    )
    .await;
    let foreign_category_id = create_taxonomy_term(
        &taxonomy,
        other_tenant_id,
        TaxonomyTermKind::Category,
        "Foreign Blog Taxonomy Category",
    )
    .await;

    let first = binding
        .bind(tenant_id, blog_category_id, taxonomy_category_id)
        .await
        .expect("same-tenant Category binding should succeed");
    assert_eq!(first.blog_category_id, blog_category_id);
    assert_eq!(first.taxonomy_category_id, taxonomy_category_id);

    binding
        .bind(tenant_id, blog_category_id, taxonomy_category_id)
        .await
        .expect("repeating the same binding should be idempotent");

    assert!(
        binding
            .bind(tenant_id, blog_category_id, second_taxonomy_category_id)
            .await
            .is_err(),
        "a Blog category must not be rebound implicitly"
    );
    assert!(
        binding
            .bind(tenant_id, second_blog_category_id, taxonomy_category_id)
            .await
            .is_err(),
        "one Taxonomy Category must not bind to two Blog categories in one tenant"
    );
    assert!(
        binding
            .bind(tenant_id, second_blog_category_id, tag_id)
            .await
            .is_err(),
        "Taxonomy Tags must not masquerade as Categories"
    );
    assert!(
        binding
            .bind(tenant_id, second_blog_category_id, foreign_category_id)
            .await
            .is_err(),
        "foreign-tenant Taxonomy Categories must fail closed"
    );
    assert!(
        binding
            .bind(tenant_id, second_blog_category_id, Uuid::new_v4())
            .await
            .is_err(),
        "stale Taxonomy Category identities must fail closed"
    );

    let rows = blog_category_taxonomy_binding::Entity::find()
        .all(&db)
        .await
        .expect("binding rows should load");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tenant_id, tenant_id);
    assert_eq!(rows[0].blog_category_id, blog_category_id);
    assert_eq!(rows[0].taxonomy_category_id, taxonomy_category_id);
}
