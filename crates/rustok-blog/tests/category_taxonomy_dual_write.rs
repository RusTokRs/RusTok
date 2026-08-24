use std::sync::Arc;

use rustok_blog::{
    BlogCategoryTaxonomyBindingEntity, BlogError, BlogModule, CategoryService, CreateCategoryInput,
    UpdateCategoryInput,
    entities::{blog_category, blog_category_taxonomy_binding},
};
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{
    SyncModuleCategoryInput, TaxonomyModule,
    entities::{taxonomy_term, taxonomy_term_alias, taxonomy_term_translation},
    sync_module_category_with_owned_aliases_in_tx,
};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, TransactionTrait,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> DatabaseConnection {
    let db = rustok_test_utils::db::setup_test_db().await;
    let manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in BlogModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("blog migration should apply");
    }
    db
}

fn service(db: &DatabaseConnection) -> CategoryService {
    CategoryService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(MemoryTransport::new())),
    )
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn create_input(name: &str, slug: &str) -> CreateCategoryInput {
    CreateCategoryInput {
        locale: "en".to_string(),
        name: name.to_string(),
        slug: Some(slug.to_string()),
        description: Some(format!("{name} description")),
        parent_id: None,
        position: Some(0),
        settings: serde_json::json!({"layout": "blog-owned"}),
    }
}

#[tokio::test]
async fn category_create_and_update_dual_write_copy_routes_and_binding() {
    let db = setup().await;
    let service = service(&db);
    let tenant_id = Uuid::new_v4();

    let category_id = service
        .create(tenant_id, admin(), create_input("Support", "support"))
        .await
        .expect("Blog category create should dual-write to Taxonomy");

    let term = taxonomy_term::Entity::find_by_id(category_id)
        .one(&db)
        .await
        .expect("Taxonomy identity read should succeed")
        .expect("same-ID Taxonomy Category should exist");
    assert_eq!(term.tenant_id, tenant_id);
    assert_eq!(term.scope_value, "blog");
    assert_eq!(term.canonical_key, format!("blog-category-{category_id}"));

    let initial = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_translation::Column::TermId.eq(category_id))
        .filter(taxonomy_term_translation::Column::Locale.eq("en"))
        .one(&db)
        .await
        .expect("Taxonomy localized copy read should succeed")
        .expect("Taxonomy localized copy should exist");
    assert_eq!(initial.name, "Support");
    assert_eq!(initial.slug, "support");

    let binding = BlogCategoryTaxonomyBindingEntity::find_by_id((tenant_id, category_id))
        .one(&db)
        .await
        .expect("binding read should succeed")
        .expect("same-ID Blog Taxonomy binding should exist");
    assert_eq!(binding.blog_category_id, category_id);
    assert_eq!(binding.taxonomy_category_id, category_id);

    service
        .update(
            tenant_id,
            category_id,
            admin(),
            UpdateCategoryInput {
                locale: "en".to_string(),
                name: Some("Help".to_string()),
                slug: Some("help".to_string()),
                description: Some("Help centre".to_string()),
                position: None,
                settings: None,
            },
        )
        .await
        .expect("Blog category update should dual-write to Taxonomy");

    let updated = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_translation::Column::TermId.eq(category_id))
        .filter(taxonomy_term_translation::Column::Locale.eq("en"))
        .one(&db)
        .await
        .expect("updated Taxonomy copy read should succeed")
        .expect("updated Taxonomy copy should exist");
    assert_eq!(updated.name, "Help");
    assert_eq!(updated.slug, "help");
    assert_eq!(updated.description.as_deref(), Some("Help centre"));

    let old_route = taxonomy_term_alias::Entity::find()
        .filter(taxonomy_term_alias::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_alias::Column::TermId.eq(category_id))
        .filter(taxonomy_term_alias::Column::Locale.eq("en"))
        .filter(taxonomy_term_alias::Column::Slug.eq("support"))
        .one(&db)
        .await
        .expect("Taxonomy alias read should succeed");
    assert!(old_route.is_some(), "Taxonomy must own historical Blog route aliases");
}

#[tokio::test]
async fn taxonomy_route_conflict_rolls_back_blog_create() {
    let db = setup().await;
    let service = service(&db);
    let tenant_id = Uuid::new_v4();
    let taxonomy_owner_id = Uuid::new_v4();

    let txn = db.begin().await.expect("Taxonomy owner transaction should begin");
    sync_module_category_with_owned_aliases_in_tx(
        &txn,
        tenant_id,
        SyncModuleCategoryInput {
            category_id: taxonomy_owner_id,
            module_scope: "blog".to_string(),
            canonical_key: format!("blog-route-owner-{taxonomy_owner_id}"),
            locale: "en".to_string(),
            name: "Route owner".to_string(),
            slug: "route-owner".to_string(),
            aliases: vec!["reserved".to_string()],
            description: None,
            parent_id: None,
            position: 0,
            icon_key: None,
            color: None,
        },
    )
    .await
    .expect("Taxonomy route owner should be created");
    txn.commit()
        .await
        .expect("Taxonomy route owner transaction should commit");

    let before_categories = blog_category::Entity::find()
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .count(&db)
        .await
        .expect("Blog category count should succeed");
    let before_bindings = blog_category_taxonomy_binding::Entity::find()
        .filter(blog_category_taxonomy_binding::Column::TenantId.eq(tenant_id))
        .count(&db)
        .await
        .expect("Blog binding count should succeed");

    let error = service
        .create(tenant_id, admin(), create_input("Reserved", "reserved"))
        .await
        .expect_err("Taxonomy historical route ownership must reject Blog create");
    assert!(matches!(error, BlogError::Validation(_)));

    assert_eq!(
        blog_category::Entity::find()
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .expect("Blog category count after rollback should succeed"),
        before_categories,
        "failed Taxonomy synchronization must roll back the Blog category row"
    );
    assert_eq!(
        blog_category_taxonomy_binding::Entity::find()
            .filter(blog_category_taxonomy_binding::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .expect("Blog binding count after rollback should succeed"),
        before_bindings,
        "failed Taxonomy synchronization must not leak a Blog binding"
    );
}
