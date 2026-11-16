use std::collections::HashMap;
use std::{sync::Arc, time::Duration};

use flex::{
    CreateFieldDefinitionCommand, FieldDefinitionService, FlexModule,
    GenericAttachedFieldDefinitionService, TAXONOMY_CATEGORY_ENTITY_TYPE,
    UpdateFieldDefinitionCommand,
};
use flex::{
    FieldDefRegistry, FlexAttachedFieldPolicy, FlexAttachedFieldPolicyReadPort,
    FlexAttachedFieldReadService, FlexAttachedSchemaReadPort, FlexDataClassification,
    upsert_attached_field_policy,
};
use rustok_api::{PortActor, PortContext};
use rustok_core::{
    MigrationSource,
    field_schema::{FieldType, FlexError},
};
use rustok_outbox::OutboxModule;
use sea_orm::{ConnectionTrait, Database, EntityTrait, PaginatorTrait};
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

async fn setup() -> sea_orm::DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite Flex fixture should connect");
    let manager = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
    }
    for migration in FlexModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("Flex migration should apply");
    }
    db
}

fn create_command(field_key: &str, is_localized: bool) -> CreateFieldDefinitionCommand {
    CreateFieldDefinitionCommand {
        field_key: field_key.to_string(),
        field_type: FieldType::Text,
        label: HashMap::from([("en".to_string(), "Label".to_string())]),
        description: None,
        is_localized,
        is_required: false,
        default_value: None,
        validation: None,
        position: None,
    }
}

#[tokio::test]
async fn published_flex_read_ports_preserve_schema_scope_and_policy_provenance() {
    let db = setup().await;
    let tenant = Uuid::new_v4();
    let store = Arc::new(GenericAttachedFieldDefinitionService::new(
        TAXONOMY_CATEGORY_ENTITY_TYPE,
    ));
    let mut active = create_command("tagline", true);
    active.default_value = Some(serde_json::json!("Default copy"));
    store.create(&db, tenant, None, active).await.unwrap();
    let (inactive, _) = store
        .create(&db, tenant, None, create_command("obsolete", false))
        .await
        .unwrap();
    store
        .deactivate(&db, tenant, None, inactive.id)
        .await
        .unwrap();
    let policy = FlexAttachedFieldPolicy {
        classification: FlexDataClassification::Public,
        ai_export_allowed: true,
    };
    upsert_attached_field_policy(
        &db,
        tenant,
        TAXONOMY_CATEGORY_ENTITY_TYPE,
        "tagline",
        policy,
    )
    .await
    .unwrap();

    let mut registry = FieldDefRegistry::new();
    registry.register(store);
    let provider = FlexAttachedFieldReadService::new(db, registry);
    let context = |tenant: Uuid| {
        PortContext::new(
            tenant.to_string(),
            PortActor::system(),
            "en",
            "flex-read-ports",
        )
        .with_deadline(Duration::from_secs(5))
    };
    let schema = provider
        .read_attached_schema(context(tenant), TAXONOMY_CATEGORY_ENTITY_TYPE)
        .await
        .unwrap();
    schema
        .verify_scope(tenant, TAXONOMY_CATEGORY_ENTITY_TYPE)
        .unwrap();
    assert_eq!(schema.definitions.len(), 1);
    assert!(schema.definitions[0].is_localized);
    assert_eq!(
        schema.definitions[0].default_value,
        Some(serde_json::json!("Default copy"))
    );

    let other = Uuid::new_v4();
    assert!(
        provider
            .read_attached_schema(context(other), TAXONOMY_CATEGORY_ENTITY_TYPE)
            .await
            .unwrap()
            .definitions
            .is_empty()
    );
    let keys = ["tagline".into(), "obsolete".into()];
    let resolutions = provider
        .read_attached_field_policies(context(tenant), TAXONOMY_CATEGORY_ENTITY_TYPE, &keys)
        .await
        .unwrap();
    assert_eq!(resolutions["tagline"].policy, policy);
    assert!(resolutions["tagline"].explicit);
    assert_eq!(resolutions["obsolete"], Default::default());
    let other_resolutions = provider
        .read_attached_field_policies(context(other), TAXONOMY_CATEGORY_ENTITY_TYPE, &keys)
        .await
        .unwrap();
    assert_eq!(other_resolutions["tagline"], Default::default());
    let mut invalid = context(tenant);
    invalid.deadline_ms = None;
    assert_eq!(
        provider
            .read_attached_schema(invalid, TAXONOMY_CATEGORY_ENTITY_TYPE)
            .await
            .unwrap_err()
            .code,
        "port.deadline_required"
    );
}

#[tokio::test]
async fn generic_definition_service_is_tenant_scoped_and_reuses_flex_guards() {
    let db = setup().await;
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let service = GenericAttachedFieldDefinitionService::new(TAXONOMY_CATEGORY_ENTITY_TYPE);

    let (created_a, event_a) = service
        .create(
            &db,
            tenant_a,
            Some(Uuid::new_v4()),
            create_command("tagline", true),
        )
        .await
        .expect("tenant A definition should create");
    assert_eq!(created_a.field_key, "tagline");
    assert_eq!(event_a.tenant_id, tenant_a);
    let stored_event = rustok_outbox::entity::Entity::find_by_id(event_a.id)
        .one(&db)
        .await
        .expect("outbox query should succeed")
        .expect("returned event should be persisted");
    assert_eq!(
        stored_event.payload,
        serde_json::to_value(&event_a).unwrap()
    );
    assert_eq!(
        rustok_outbox::entity::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        1
    );

    let tenant_b_before = service
        .list_all(&db, tenant_b)
        .await
        .expect("tenant B list should succeed");
    assert!(tenant_b_before.is_empty());

    service
        .create(
            &db,
            tenant_b,
            Some(Uuid::new_v4()),
            create_command("tagline", true),
        )
        .await
        .expect("same key should be allowed in another tenant");

    let duplicate = service
        .create(
            &db,
            tenant_a,
            Some(Uuid::new_v4()),
            create_command("tagline", true),
        )
        .await
        .expect_err("same key in the same tenant should be rejected");
    assert!(matches!(duplicate, FlexError::DuplicateFieldKey(key) if key == "tagline"));

    let schema_a = service
        .get_schema(&db, tenant_a)
        .await
        .expect("tenant A schema should load");
    let active = schema_a.active_definitions();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].field_key, "tagline");
    assert!(active[0].is_localized);
}

#[tokio::test]
async fn generic_definition_mutations_persist_the_returned_envelope_identity() {
    let db = setup().await;
    let tenant = Uuid::new_v4();
    let actor = Some(Uuid::new_v4());
    let service = GenericAttachedFieldDefinitionService::new(TAXONOMY_CATEGORY_ENTITY_TYPE);
    let (created, created_event) = service
        .create(&db, tenant, actor, create_command("tagline", true))
        .await
        .expect("create should commit");
    let (_, updated_event) = service
        .update(
            &db,
            tenant,
            actor,
            created.id,
            UpdateFieldDefinitionCommand {
                is_required: Some(true),
                ..Default::default()
            },
        )
        .await
        .expect("update should commit");
    let deactivated_event = service
        .deactivate(&db, tenant, actor, created.id)
        .await
        .expect("deactivation should commit");

    for event in [created_event, updated_event, deactivated_event] {
        let stored = rustok_outbox::entity::Entity::find_by_id(event.id)
            .one(&db)
            .await
            .expect("outbox query should succeed")
            .expect("returned envelope identity should be durable");
        assert_eq!(stored.payload, serde_json::to_value(&event).unwrap());
    }
    assert_eq!(
        rustok_outbox::entity::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        3
    );
}

#[tokio::test]
async fn generic_definition_mutations_roll_back_when_outbox_is_unavailable() {
    let db = setup().await;
    let tenant = Uuid::new_v4();
    let service = GenericAttachedFieldDefinitionService::new(TAXONOMY_CATEGORY_ENTITY_TYPE);
    let (created, _) = service
        .create(&db, tenant, None, create_command("tagline", true))
        .await
        .expect("initial definition should commit");
    db.execute_unprepared("DROP TABLE sys_events")
        .await
        .expect("fixture should remove outbox");

    assert!(matches!(
        service
            .create(&db, tenant, None, create_command("other_field", false))
            .await,
        Err(FlexError::Database(_))
    ));
    assert!(matches!(
        service
            .update(
                &db,
                tenant,
                None,
                created.id,
                UpdateFieldDefinitionCommand {
                    is_required: Some(true),
                    ..Default::default()
                },
            )
            .await,
        Err(FlexError::Database(_))
    ));
    assert!(matches!(
        service.deactivate(&db, tenant, None, created.id).await,
        Err(FlexError::Database(_))
    ));

    let definitions = service.list_all(&db, tenant).await.unwrap();
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].id, created.id);
    assert!(!definitions[0].is_required);
    assert!(definitions[0].is_active);
}
