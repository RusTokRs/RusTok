use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumError, ForumModule,
    ForumTopicMergeService, MergeForumTopicInput, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn setup() -> TestResult<(DatabaseConnection, TransactionalEventBus)> {
    let db_url = format!(
        "sqlite:file:forum_topic_canonical_resolution_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    db.execute_unprepared(
        "CREATE TABLE users (\
            id TEXT NOT NULL PRIMARY KEY, \
            tenant_id TEXT NOT NULL, \
            UNIQUE (tenant_id, id)\
        )",
    )
    .await?;
    let schema = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration.up(&schema).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&schema).await?;
    }
    for migration in ForumModule.migrations() {
        migration.up(&schema).await?;
    }
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    Ok((db, event_bus))
}

async fn insert_user(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) -> TestResult<()> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
        vec![user_id.into(), tenant_id.into()],
    ))
    .await?;
    Ok(())
}

async fn create_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    security: SecurityContext,
) -> TestResult<Uuid> {
    Ok(CategoryService::new(db.clone())
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Canonical resolution".to_string(),
                slug: "canonical-resolution".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?
        .id)
}

async fn create_topic(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    category_id: Uuid,
    security: SecurityContext,
    key: &str,
) -> TestResult<Uuid> {
    Ok(TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id,
                title: format!("Canonical {key}"),
                slug: Some(format!("canonical-{key}")),
                body: rustok_api::RichTextDocument::single_paragraph(format!(
                    "Canonical {key} body"
                )),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id)
}

async fn insert_direct_merge_receipt(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    operation_id: Uuid,
    source_topic_id: Uuid,
    target_topic_id: Uuid,
    category_id: Uuid,
    actor_id: Uuid,
) -> Result<(), sea_orm::DbErr> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO forum_topic_merge_operations (\
            tenant_id, operation_id, source_topic_id, target_topic_id, category_id, actor_id, \
            reason, moved_reply_count, moved_published_reply_count, \
            resulting_published_reply_count, position_offset, event_id, merged_at\
        ) VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0, 0, 0, ?, CURRENT_TIMESTAMP)",
        vec![
            tenant_id.into(),
            operation_id.into(),
            source_topic_id.into(),
            target_topic_id.into(),
            category_id.into(),
            actor_id.into(),
            "Direct canonical edge probe".into(),
            operation_id.into(),
        ],
    ))
    .await?;
    Ok(())
}

#[tokio::test]
async fn merged_topic_ids_resolve_to_one_visible_canonical_target() -> TestResult<()> {
    let (db, event_bus) = setup().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    insert_user(&db, tenant_id, actor_id).await?;
    let admin = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let category_id = create_category(&db, tenant_id, admin.clone()).await?;
    let topic_a = create_topic(&db, &event_bus, tenant_id, category_id, admin.clone(), "a").await?;
    let topic_b = create_topic(&db, &event_bus, tenant_id, category_id, admin.clone(), "b").await?;
    let topic_c = create_topic(&db, &event_bus, tenant_id, category_id, admin.clone(), "c").await?;
    let active_topic = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "active-probe",
    )
    .await?;

    let operation_ab = Uuid::new_v4();
    ForumTopicMergeService::new(db.clone(), event_bus.clone())
        .merge_topic(
            tenant_id,
            topic_b,
            admin.clone(),
            MergeForumTopicInput {
                operation_id: operation_ab,
                source_topic_id: topic_a,
                reason: "Collapse the first duplicate topic".to_string(),
            },
        )
        .await?;

    let operation_bc = Uuid::new_v4();
    ForumTopicMergeService::new(db.clone(), event_bus.clone())
        .merge_topic(
            tenant_id,
            topic_c,
            admin.clone(),
            MergeForumTopicInput {
                operation_id: operation_bc,
                source_topic_id: topic_b,
                reason: "Collapse the intermediate duplicate topic".to_string(),
            },
        )
        .await?;

    let service = TopicService::new(db.clone(), event_bus);
    let resolution_a = service
        .resolve_canonical_topic(tenant_id, admin.clone(), topic_a)
        .await?;
    assert_eq!(resolution_a.requested_topic_id, topic_a);
    assert_eq!(resolution_a.canonical_topic_id, topic_c);
    assert!(resolution_a.redirected);
    assert_eq!(resolution_a.hop_count, 2);
    assert_eq!(
        resolution_a.merge_operation_ids,
        vec![operation_ab, operation_bc]
    );

    let resolution_b = service
        .resolve_canonical_topic(tenant_id, admin.clone(), topic_b)
        .await?;
    assert_eq!(resolution_b.canonical_topic_id, topic_c);
    assert_eq!(resolution_b.hop_count, 1);
    assert_eq!(resolution_b.merge_operation_ids, vec![operation_bc]);

    let resolution_c = service
        .resolve_canonical_topic(tenant_id, admin.clone(), topic_c)
        .await?;
    assert_eq!(resolution_c.canonical_topic_id, topic_c);
    assert!(!resolution_c.redirected);
    assert_eq!(resolution_c.hop_count, 0);
    assert!(resolution_c.merge_operation_ids.is_empty());

    let (selected_resolution, selected) = service
        .get_with_canonical_resolution_and_locale_fallback(
            tenant_id,
            admin.clone(),
            topic_a,
            "en",
            None,
        )
        .await?;
    assert_eq!(selected_resolution, resolution_a);
    assert_eq!(selected.id, topic_c);
    assert_eq!(selected.title, "Canonical c");

    let storefront = service
        .get_storefront_visible_with_locale_fallback(
            tenant_id,
            admin.clone(),
            topic_a,
            "en",
            None,
            None,
        )
        .await?
        .ok_or("canonical storefront topic missing")?;
    assert_eq!(storefront.id, topic_c);

    let missing_id = Uuid::new_v4();
    assert!(matches!(
        service
            .resolve_canonical_topic(tenant_id, admin.clone(), missing_id)
            .await,
        Err(ForumError::TopicNotFound(id)) if id == missing_id
    ));

    assert!(
        insert_direct_merge_receipt(
            &db,
            tenant_id,
            Uuid::new_v4(),
            topic_a,
            topic_c,
            category_id,
            actor_id,
        )
        .await
        .is_err()
    );

    assert!(
        insert_direct_merge_receipt(
            &db,
            tenant_id,
            Uuid::new_v4(),
            active_topic,
            topic_c,
            category_id,
            actor_id,
        )
        .await
        .is_err()
    );

    Ok(())
}
