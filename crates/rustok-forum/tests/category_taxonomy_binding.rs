use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    ForumCategoryTaxonomyBindingService, ForumModule, entities::forum_category_taxonomy_binding,
};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyModule, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, EntityTrait,
    Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const FORUM_BINDING_SCHEMA_MIGRATIONS: &[&str] = &[
    "m20260328_000001_create_forum_tables",
    "m20260712_000001_enforce_forum_core_tenant_integrity",
    "m20260823_000029_add_forum_taxonomy_category_binding",
];

async fn setup() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:forum_category_taxonomy_binding_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(opts)
        .await
        .expect("forum Taxonomy binding database should connect");
    let schema = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("taxonomy migration should apply");
    }

    let mut applied_forum_migrations = 0usize;
    for migration in ForumModule.migrations() {
        if FORUM_BINDING_SCHEMA_MIGRATIONS.contains(&migration.name()) {
            migration
                .up(&schema)
                .await
                .expect("required Forum binding migration should apply");
            applied_forum_migrations += 1;
        }
    }
    assert_eq!(
        applied_forum_migrations,
        FORUM_BINDING_SCHEMA_MIGRATIONS.len(),
        "every required Forum binding migration must be registered"
    );
    db
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn create_forum_category(db: &DatabaseConnection, tenant_id: Uuid) -> Uuid {
    let category_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO forum_categories (id, tenant_id) VALUES (?, ?)",
        [category_id.into(), tenant_id.into()],
    ))
    .await
    .expect("Forum category owner row should be created");
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
async fn forum_category_binding_is_category_only_tenant_bounded_and_one_to_one() {
    let db = setup().await;
    let taxonomy = TaxonomyService::new(db.clone());
    let binding = ForumCategoryTaxonomyBindingService::new(db.clone());

    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let forum_category_id = create_forum_category(&db, tenant_id).await;
    let second_forum_category_id = create_forum_category(&db, tenant_id).await;

    let taxonomy_category_id = create_taxonomy_term(
        &taxonomy,
        tenant_id,
        TaxonomyTermKind::Category,
        "General Taxonomy Category",
    )
    .await;
    let second_taxonomy_category_id = create_taxonomy_term(
        &taxonomy,
        tenant_id,
        TaxonomyTermKind::Category,
        "Support Taxonomy Category",
    )
    .await;
    let tag_id = create_taxonomy_term(
        &taxonomy,
        tenant_id,
        TaxonomyTermKind::Tag,
        "Forum binding tag",
    )
    .await;
    let foreign_category_id = create_taxonomy_term(
        &taxonomy,
        other_tenant_id,
        TaxonomyTermKind::Category,
        "Foreign Taxonomy Category",
    )
    .await;

    let first = binding
        .bind(tenant_id, forum_category_id, taxonomy_category_id)
        .await
        .expect("same-tenant Category binding should succeed");
    assert_eq!(first.forum_category_id, forum_category_id);
    assert_eq!(first.taxonomy_category_id, taxonomy_category_id);

    binding
        .bind(tenant_id, forum_category_id, taxonomy_category_id)
        .await
        .expect("repeating the same binding should be idempotent");

    assert!(
        binding
            .bind(tenant_id, forum_category_id, second_taxonomy_category_id)
            .await
            .is_err(),
        "a Forum category must not be rebound implicitly"
    );
    assert!(
        binding
            .bind(tenant_id, second_forum_category_id, taxonomy_category_id)
            .await
            .is_err(),
        "one Taxonomy Category must not bind to two Forum categories in one tenant"
    );
    assert!(
        binding
            .bind(tenant_id, second_forum_category_id, tag_id)
            .await
            .is_err(),
        "Taxonomy Tags must not masquerade as Categories"
    );
    assert!(
        binding
            .bind(tenant_id, second_forum_category_id, foreign_category_id)
            .await
            .is_err(),
        "foreign-tenant Taxonomy Categories must fail closed"
    );
    assert!(
        binding
            .bind(tenant_id, second_forum_category_id, Uuid::new_v4())
            .await
            .is_err(),
        "stale Taxonomy Category identities must fail closed"
    );

    let rows = forum_category_taxonomy_binding::Entity::find()
        .all(&db)
        .await
        .expect("binding rows should load");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tenant_id, tenant_id);
    assert_eq!(rows[0].forum_category_id, forum_category_id);
    assert_eq!(rows[0].taxonomy_category_id, taxonomy_category_id);
}
