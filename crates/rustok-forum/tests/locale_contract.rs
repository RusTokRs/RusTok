use std::sync::Arc;

use flex::attached;
use rustok_core::field_schema::FieldType;
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    forum_topic, CategoryService, CreateCategoryInput, CreateTopicInput, ForumError, ForumModule,
    ListTopicsFilter, TopicService, UpdateCategoryInput, UpdateTopicInput,
};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, QueryFilter, Schema, Set,
};
use sea_orm_migration::SchemaManager;
use tokio::sync::broadcast;
use uuid::Uuid;

mod topic_field_definitions_storage {
    rustok_core::define_field_definitions_entity!("topic_field_definitions");
}

async fn setup_forum_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:forum_locale_contract_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);

    Database::connect(opts)
        .await
        .expect("failed to connect forum sqlite database")
}

async fn setup() -> (
    DatabaseConnection,
    TransactionalEventBus,
    broadcast::Receiver<rustok_events::EventEnvelope>,
    Uuid,
) {
    let db = setup_forum_test_db().await;
    let schema = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("taxonomy migration should apply");
    }
    let module = ForumModule;
    for migration in module.migrations() {
        migration
            .up(&schema)
            .await
            .expect("forum migration should apply");
    }
    ensure_topic_flex_schema(&db).await;

    let transport = MemoryTransport::new();
    let receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    (db, event_bus, receiver, Uuid::new_v4())
}

async fn ensure_topic_flex_schema(db: &DatabaseConnection) {
    if db.get_database_backend() != DbBackend::Sqlite {
        return;
    }

    let builder = db.get_database_backend();
    let schema = Schema::new(builder);
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(topic_field_definitions_storage::Entity),
    )
    .await;
    let attached_table = sea_orm::sea_query::Table::create()
        .table(sea_orm::sea_query::Alias::new(
            "flex_attached_localized_values",
        ))
        .if_not_exists()
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("id"))
                .uuid()
                .not_null()
                .primary_key(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("tenant_id"))
                .uuid()
                .not_null(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("entity_type"))
                .string_len(64)
                .not_null(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("entity_id"))
                .uuid()
                .not_null(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("field_key"))
                .string_len(128)
                .not_null(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("locale"))
                .string_len(32)
                .not_null(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("value"))
                .json_binary()
                .not_null(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("created_at"))
                .timestamp_with_time_zone()
                .not_null()
                .default(sea_orm::sea_query::Expr::current_timestamp()),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("updated_at"))
                .timestamp_with_time_zone()
                .not_null()
                .default(sea_orm::sea_query::Expr::current_timestamp()),
        )
        .to_owned();
    db.execute(builder.build(&attached_table))
        .await
        .expect("flex attached localized values table should be created");
}

async fn create_entity_table(
    db: &DatabaseConnection,
    builder: &DbBackend,
    mut statement: sea_orm::sea_query::TableCreateStatement,
) {
    statement.if_not_exists();
    db.execute(builder.build(&statement))
        .await
        .expect("test table should be created");
}

#[tokio::test]
async fn category_list_exposes_requested_effective_and_available_locales() {
    let (db, _event_bus, _events, tenant_id) = setup().await;
    let service = CategoryService::new(db);
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let category = service
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "General".to_string(),
                slug: "general".to_string(),
                description: Some("General discussion".to_string()),
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category should be created");

    service
        .update(
            tenant_id,
            category.id,
            admin.clone(),
            UpdateCategoryInput {
                locale: "ru".to_string(),
                name: Some("Общее".to_string()),
                slug: Some("obschee".to_string()),
                description: Some("Общие обсуждения".to_string()),
                icon: None,
                color: None,
                position: None,
                moderated: None,
            },
        )
        .await
        .expect("ru category translation should be saved");

    let (items, total) = service
        .list_paginated_with_locale_fallback(tenant_id, admin, "fr-FR", 1, 20, Some("ru"))
        .await
        .expect("category list should load");

    assert_eq!(total, 1);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].requested_locale, "fr-FR");
    assert_eq!(items[0].locale, "fr-FR");
    assert_eq!(items[0].effective_locale, "ru");
    assert_eq!(
        items[0].available_locales,
        vec!["en".to_string(), "ru".to_string()]
    );
    assert_eq!(items[0].name, "Общее");
    assert_eq!(items[0].slug, "obschee");
}

#[tokio::test]
async fn topic_list_exposes_requested_effective_and_available_locales() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db, event_bus);
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let category = category_service
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "General".to_string(),
                slug: "general".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category should be created");

    let topic = topic_service
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "General thread".to_string(),
                slug: Some("general-thread".to_string()),
                body: "English body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");

    topic_service
        .update(
            tenant_id,
            topic.id,
            admin.clone(),
            UpdateTopicInput {
                locale: "ru".to_string(),
                title: Some("Общая тема".to_string()),
                body: Some("Русское тело".to_string()),
                body_format: Some("markdown".to_string()),
                content_json: None,
                metadata: None,
                tags: None,
                channel_slugs: None,
            },
        )
        .await
        .expect("ru topic translation should be saved");

    let (items, total) = topic_service
        .list_with_locale_fallback(
            tenant_id,
            admin,
            ListTopicsFilter {
                category_id: Some(category.id),
                status: None,
                locale: Some("fr-FR".to_string()),
                page: 1,
                per_page: 20,
            },
            Some("ru"),
        )
        .await
        .expect("topic list should load");

    assert_eq!(total, 1);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].requested_locale, "fr-FR");
    assert_eq!(items[0].locale, "fr-FR");
    assert_eq!(items[0].effective_locale, "ru");
    assert_eq!(
        items[0].available_locales,
        vec!["en".to_string(), "ru".to_string()]
    );
    assert_eq!(items[0].title, "Общая тема");
    assert_eq!(items[0].slug, "general-thread");
}

#[tokio::test]
async fn topic_resolves_localized_flex_metadata_from_attached_values() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus);
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let string_field_type = serde_json::to_value(FieldType::Text)
        .expect("field type should serialize")
        .as_str()
        .expect("field type should be string")
        .to_string();

    topic_field_definitions_storage::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        field_key: Set("summary".to_string()),
        field_type: Set(string_field_type.clone()),
        label: Set(serde_json::json!({ "en": "Summary", "ru": "Сводка" })),
        description: Set(None),
        is_localized: Set(true),
        is_required: Set(false),
        default_value: Set(None),
        validation: Set(None),
        position: Set(0),
        is_active: Set(true),
        created_at: sea_orm::ActiveValue::NotSet,
        updated_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(&db)
    .await
    .expect("localized field definition should be created");

    topic_field_definitions_storage::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        field_key: Set("audience".to_string()),
        field_type: Set(string_field_type),
        label: Set(serde_json::json!({ "en": "Audience", "ru": "Аудитория" })),
        description: Set(None),
        is_localized: Set(false),
        is_required: Set(false),
        default_value: Set(None),
        validation: Set(None),
        position: Set(1),
        is_active: Set(true),
        created_at: sea_orm::ActiveValue::NotSet,
        updated_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(&db)
    .await
    .expect("shared field definition should be created");

    let category = category_service
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "General".to_string(),
                slug: "general".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category should be created");

    let topic = topic_service
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Localized flex".to_string(),
                slug: Some("localized-flex".to_string()),
                body: "English body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                metadata: serde_json::json!({
                    "summary": "English summary",
                    "audience": "everyone"
                }),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");

    topic_service
        .update(
            tenant_id,
            topic.id,
            admin.clone(),
            UpdateTopicInput {
                locale: "ru".to_string(),
                title: Some("Локализованный flex".to_string()),
                body: Some("Русское тело".to_string()),
                body_format: Some("markdown".to_string()),
                content_json: None,
                metadata: Some(serde_json::json!({
                    "summary": "Русская сводка",
                    "audience": "everyone"
                })),
                tags: None,
                channel_slugs: None,
            },
        )
        .await
        .expect("ru translation should be saved");

    let en_topic = topic_service
        .get_with_locale_fallback(tenant_id, admin.clone(), topic.id, "en", Some("ru"))
        .await
        .expect("en topic should load");
    assert_eq!(en_topic.metadata["summary"], "English summary");
    assert_eq!(en_topic.metadata["audience"], "everyone");

    let stored_topic = forum_topic::Entity::find_by_id(topic.id)
        .one(&db)
        .await
        .expect("topic query should succeed")
        .expect("stored topic should exist");
    assert_eq!(
        stored_topic.metadata,
        serde_json::json!({ "audience": "everyone" })
    );

    let localized_rows = attached::Entity::find()
        .filter(attached::Column::TenantId.eq(tenant_id))
        .filter(attached::Column::EntityType.eq("topic"))
        .filter(attached::Column::EntityId.eq(topic.id))
        .all(&db)
        .await
        .expect("localized rows query should succeed");
    assert_eq!(localized_rows.len(), 2);
    assert!(localized_rows.iter().any(|row| {
        row.field_key == "summary"
            && row.locale == "en"
            && row.value == serde_json::json!("English summary")
    }));
    assert!(localized_rows.iter().any(|row| {
        row.field_key == "summary"
            && row.locale == "ru"
            && row.value == serde_json::json!("Русская сводка")
    }));

    let fallback_topic = topic_service
        .get_with_locale_fallback(tenant_id, admin, topic.id, "fr-FR", Some("ru"))
        .await
        .expect("fallback topic should load");
    assert_eq!(fallback_topic.metadata["summary"], "Русская сводка");
    assert_eq!(fallback_topic.metadata["audience"], "everyone");
}

#[tokio::test]
async fn topic_create_applies_shared_defaults_and_persists_localized_attached_values() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus);
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let string_field_type = serde_json::to_value(FieldType::Text)
        .expect("field type should serialize")
        .as_str()
        .expect("field type should be string")
        .to_string();

    topic_field_definitions_storage::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        field_key: Set("summary".to_string()),
        field_type: Set(string_field_type.clone()),
        label: Set(serde_json::json!({ "en": "Summary" })),
        description: Set(None),
        is_localized: Set(true),
        is_required: Set(false),
        default_value: Set(None),
        validation: Set(None),
        position: Set(0),
        is_active: Set(true),
        created_at: sea_orm::ActiveValue::NotSet,
        updated_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(&db)
    .await
    .expect("localized field definition should be created");

    topic_field_definitions_storage::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        field_key: Set("audience".to_string()),
        field_type: Set(string_field_type),
        label: Set(serde_json::json!({ "en": "Audience" })),
        description: Set(None),
        is_localized: Set(false),
        is_required: Set(false),
        default_value: Set(Some(serde_json::json!("everyone"))),
        validation: Set(None),
        position: Set(1),
        is_active: Set(true),
        created_at: sea_orm::ActiveValue::NotSet,
        updated_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(&db)
    .await
    .expect("shared field definition should be created");

    let category = category_service
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "General".to_string(),
                slug: "general".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category should be created");

    let topic = topic_service
        .create(
            tenant_id,
            admin,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Defaults".to_string(),
                slug: Some("defaults".to_string()),
                body: "English body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                metadata: serde_json::json!({
                    "summary": "English summary",
                    "unknown_custom": "keep-me"
                }),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");

    assert_eq!(topic.metadata["summary"], "English summary");
    assert_eq!(topic.metadata["audience"], "everyone");
    assert_eq!(topic.metadata["unknown_custom"], "keep-me");

    let stored_topic = forum_topic::Entity::find_by_id(topic.id)
        .one(&db)
        .await
        .expect("topic query should succeed")
        .expect("stored topic should exist");
    assert_eq!(
        stored_topic.metadata,
        serde_json::json!({
            "unknown_custom": "keep-me",
            "audience": "everyone"
        })
    );

    let localized_rows = attached::Entity::find()
        .filter(attached::Column::TenantId.eq(tenant_id))
        .filter(attached::Column::EntityType.eq("topic"))
        .filter(attached::Column::EntityId.eq(topic.id))
        .all(&db)
        .await
        .expect("localized rows query should succeed");
    assert_eq!(localized_rows.len(), 1);
    assert_eq!(localized_rows[0].field_key, "summary");
    assert_eq!(localized_rows[0].locale, "en");
    assert_eq!(
        localized_rows[0].value,
        serde_json::json!("English summary")
    );
}

#[tokio::test]
async fn topic_create_rejects_missing_required_localized_custom_field() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus);
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let string_field_type = serde_json::to_value(FieldType::Text)
        .expect("field type should serialize")
        .as_str()
        .expect("field type should be string")
        .to_string();

    topic_field_definitions_storage::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        field_key: Set("summary".to_string()),
        field_type: Set(string_field_type),
        label: Set(serde_json::json!({ "en": "Summary" })),
        description: Set(None),
        is_localized: Set(true),
        is_required: Set(true),
        default_value: Set(None),
        validation: Set(None),
        position: Set(0),
        is_active: Set(true),
        created_at: sea_orm::ActiveValue::NotSet,
        updated_at: sea_orm::ActiveValue::NotSet,
    }
    .insert(&db)
    .await
    .expect("required localized field definition should be created");

    let category = category_service
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "General".to_string(),
                slug: "general".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category should be created");

    let error = topic_service
        .create(
            tenant_id,
            admin,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Missing summary".to_string(),
                slug: Some("missing-summary".to_string()),
                body: "English body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect_err("topic creation should fail without required localized custom field");

    match error {
        ForumError::Validation(message) => {
            assert!(
                message.contains("Custom field validation failed"),
                "unexpected error: {message}"
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}
