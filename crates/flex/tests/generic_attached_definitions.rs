use std::collections::HashMap;

use flex::{
    CreateFieldDefinitionCommand, FieldDefinitionService, FlexModule,
    GenericAttachedFieldDefinitionService, TAXONOMY_CATEGORY_ENTITY_TYPE,
};
use rustok_core::{MigrationSource, field_schema::{FieldType, FlexError}};
use sea_orm::Database;
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

async fn setup() -> sea_orm::DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite Flex fixture should connect");
    let manager = SchemaManager::new(&db);
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
