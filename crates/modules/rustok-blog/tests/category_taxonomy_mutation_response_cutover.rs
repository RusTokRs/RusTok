use std::sync::Arc;

use rustok_api::{Action, Permission, Resource};
use rustok_blog::{BlogModule, CategoryService, CreateCategoryInput, UpdateCategoryInput};
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_events::EventEnvelope;
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_taxonomy::{
    SyncModuleCategoryInput, TaxonomyModule, sync_module_category_with_owned_aliases_in_tx,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TransactionTrait};
use sea_orm_migration::{MigrationTrait, SchemaManager};
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
async fn response_projection_failure_rolls_back_category_copy_revision_and_outbox() {
    let db = setup().await;
    SysEventsMigration
        .up(&SchemaManager::new(&db))
        .await
        .unwrap();
    let service = CategoryService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
    );
    let tenant_id = Uuid::new_v4();
    let category_id = service
        .create(
            tenant_id,
            admin(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "Original".into(),
                slug: Some("original".into()),
                description: Some("Original description".into()),
                parent_id: None,
                position: Some(0),
                settings: serde_json::json!({"layout": "original"}),
            },
        )
        .await
        .unwrap();
    let before = category_transaction_state(&db, category_id).await;
    // Inject damage after the command's last durable side effect. A response
    // read outside the transaction would report failure after committing it.
    db.execute_unprepared(
        "CREATE TRIGGER damage_category_projection AFTER INSERT ON sys_events \
         BEGIN DELETE FROM taxonomy_category_hierarchy; END",
    )
    .await
    .unwrap();

    let result = service
        .update(
            tenant_id,
            category_id,
            update_only(),
            UpdateCategoryInput {
                locale: "en".into(),
                name: Some("Changed".into()),
                slug: Some("changed".into()),
                description: rustok_api::Patch::Set("Changed description".into()),
                settings: Some(serde_json::json!({"layout": "changed"})),
            },
        )
        .await;
    assert!(matches!(result, Err(rustok_blog::BlogError::Invariant(_))));
    assert_eq!(category_transaction_state(&db, category_id).await, before);
    let persisted = service
        .get(tenant_id, admin(), category_id, "en")
        .await
        .unwrap();
    assert_eq!(persisted.name, "Original");
    assert_eq!(
        persisted.description.as_deref(),
        Some("Original description")
    );
    assert_eq!(
        persisted.settings,
        serde_json::json!({"layout": "original"})
    );
}

async fn category_transaction_state(
    db: &DatabaseConnection,
    category_id: Uuid,
) -> serde_json::Value {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT revision, updated_at, settings,
            (SELECT COUNT(*) FROM taxonomy_category_hierarchy) AS hierarchy_count,
            (SELECT COUNT(*) FROM sys_events) AS outbox_count
         FROM blog_categories WHERE id = ?",
            [category_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    serde_json::json!({
        "revision": row.try_get::<i64>("", "revision").unwrap(),
        "updated_at": row.try_get::<String>("", "updated_at").unwrap(),
        "settings": row.try_get::<String>("", "settings").unwrap(),
        "hierarchy_count": row.try_get::<i64>("", "hierarchy_count").unwrap(),
        "outbox_count": row.try_get::<i64>("", "outbox_count").unwrap(),
    })
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
                description: rustok_api::Patch::Set("Help centre".to_string()),
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
