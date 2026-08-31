use std::sync::Arc;

use rustok_api::{Action, Permission, Resource};
use rustok_blog::{BlogModule, CategoryService, CreateCategoryInput, UpdateCategoryInput};
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_events::EventEnvelope;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{
    SyncModuleCategoryInput, TaxonomyModule, sync_module_category_with_owned_aliases_in_tx,
};
use sea_orm::{DatabaseConnection, TransactionTrait};
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

fn service(
    db: &DatabaseConnection,
) -> (
    CategoryService,
    tokio::sync::broadcast::Receiver<EventEnvelope>,
) {
    let transport = MemoryTransport::new();
    let receiver = transport.subscribe();
    let service = CategoryService::new(db.clone(), TransactionalEventBus::new(Arc::new(transport)));
    (service, receiver)
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn update_only() -> SecurityContext {
    SecurityContext::from_permissions(
        UserRole::Manager,
        Some(Uuid::new_v4()),
        [Permission::new(Resource::BlogCategories, Action::Update)],
    )
}

#[tokio::test]
async fn update_response_comes_from_taxonomy_without_requiring_read_permission() {
    let db = setup().await;
    let manager = SchemaManager::new(&db);
    assert!(
        !manager
            .has_table("blog_category_translations")
            .await
            .expect("legacy translation table lookup should succeed"),
        "the mutation response contract must run after donor storage retirement"
    );

    let (service, _events) = service(&db);
    let tenant_id = Uuid::new_v4();

    let category_id = service
        .create(
            tenant_id,
            admin(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Support".to_string(),
                slug: Some("support".to_string()),
                description: Some("Support description".to_string()),
                parent_id: None,
                position: Some(0),
                settings: serde_json::json!({"layout": "initial"}),
            },
        )
        .await
        .expect("Blog category should be created");

    let txn = db
        .begin()
        .await
        .expect("Taxonomy locale transaction should begin");
    sync_module_category_with_owned_aliases_in_tx(
        &txn,
        tenant_id,
        SyncModuleCategoryInput {
            category_id,
            module_scope: "blog".to_string(),
            canonical_key: format!("blog-category-{category_id}"),
            locale: "fr".to_string(),
            name: "Assistance".to_string(),
            slug: "assistance".to_string(),
            aliases: Vec::new(),
            description: Some("Copie canonique française".to_string()),
            parent_id: None,
            position: 0,
            icon_key: None,
            color: None,
        },
    )
    .await
    .expect("Taxonomy-only locale should be added");
    txn.commit()
        .await
        .expect("Taxonomy-only locale should commit");

    let restricted = update_only();
    let response = service
        .update(
            tenant_id,
            category_id,
            restricted.clone(),
            UpdateCategoryInput {
                locale: "en".to_string(),
                name: Some("Help".to_string()),
                slug: Some("help".to_string()),
                description: Some("Help centre".to_string()),
                position: None,
                settings: Some(serde_json::json!({"layout": "updated"})),
            },
        )
        .await
        .expect("Update-only authority should receive the canonical mutation response");

    assert_eq!(response.id, category_id);
    assert_eq!(response.locale, "en");
    assert_eq!(response.effective_locale, "en");
    assert_eq!(response.name, "Help");
    assert_eq!(response.slug, "help");
    assert_eq!(response.description.as_deref(), Some("Help centre"));
    assert_eq!(response.position, 0);
    assert_eq!(response.settings, serde_json::json!({"layout": "updated"}));
    assert!(
        response
            .available_locales
            .iter()
            .any(|locale| locale == "fr"),
        "mutation response must expose the Taxonomy-only locale after donor storage retirement"
    );

    let read_error = service
        .get(tenant_id, restricted, category_id, "en")
        .await
        .expect_err("the same authority must not have Blog Category read permission");
    assert!(
        matches!(read_error, rustok_blog::BlogError::Forbidden(_)),
        "update response cutover must not smuggle an additional Read ACL requirement into the mutation"
    );
}
