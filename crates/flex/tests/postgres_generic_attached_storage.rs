use std::collections::HashMap;

use flex::{
    AttachedEntityRef, CreateFieldDefinitionCommand, FieldDefinitionService, FlexModule,
    GenericAttachedFieldDefinitionService, TAXONOMY_CATEGORY_ENTITY_TYPE,
    persist_prepared_generic_attached_values, prepare_generic_attached_values_update,
    resolve_generic_attached_values,
};
use rustok_core::{
    MigrationSource,
    field_schema::{CustomFieldsSchema, FieldDefinition, FieldType},
};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use sea_orm_migration::prelude::SchemaManager;
use serde_json::json;
use uuid::Uuid;

fn definition(field_key: &str, is_localized: bool) -> FieldDefinition {
    FieldDefinition {
        field_key: field_key.to_string(),
        field_type: FieldType::Text,
        label: HashMap::from([("en".to_string(), field_key.to_string())]),
        description: None,
        is_localized,
        is_required: false,
        default_value: None,
        validation: None,
        position: 0,
        is_active: true,
    }
}

fn schema() -> CustomFieldsSchema {
    CustomFieldsSchema::new(vec![
        definition("badge", false),
        definition("tagline", true),
    ])
}

fn create_command() -> CreateFieldDefinitionCommand {
    CreateFieldDefinitionCommand {
        field_key: "tagline".to_string(),
        field_type: FieldType::Text,
        label: HashMap::from([("en".to_string(), "Tagline".to_string())]),
        description: None,
        is_localized: true,
        is_required: false,
        default_value: None,
        validation: None,
        position: None,
    }
}

async fn generation(db: &sea_orm::DatabaseConnection) -> i64 {
    db.query_one(Statement::from_string(
        DatabaseBackend::Postgres,
        "SELECT generation FROM flex_field_definition_cache_generation WHERE id = 1".to_string(),
    ))
    .await
    .expect("generation query should succeed")
    .expect("generation singleton should exist")
    .try_get("", "generation")
    .expect("generation should decode")
}

#[tokio::test]
#[ignore = "requires RUSTOK_FLEX_TEST_POSTGRES_URL"]
async fn postgres_generic_category_donor_roundtrips_and_advances_definition_generation() {
    let url = std::env::var("RUSTOK_FLEX_TEST_POSTGRES_URL")
        .expect("RUSTOK_FLEX_TEST_POSTGRES_URL must be set");
    let db = Database::connect(url)
        .await
        .expect("PostgreSQL Flex fixture should connect");
    let manager = SchemaManager::new(&db);
    for migration in FlexModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("Flex migration should apply on PostgreSQL");
    }
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS flex_attached_localized_values (\
            id UUID PRIMARY KEY, \
            tenant_id UUID NOT NULL, \
            entity_type VARCHAR(64) NOT NULL, \
            entity_id UUID NOT NULL, \
            field_key VARCHAR(128) NOT NULL, \
            locale VARCHAR(32) NOT NULL, \
            value JSONB NOT NULL, \
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, \
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, \
            UNIQUE (tenant_id, entity_type, entity_id, field_key, locale)\
        )",
    )
    .await
    .expect("localized attached storage should exist");

    let tenant_id = Uuid::new_v4();
    let entity_id = Uuid::new_v4();
    let service = GenericAttachedFieldDefinitionService::new(TAXONOMY_CATEGORY_ENTITY_TYPE);
    let generation_before = generation(&db).await;
    service
        .create(&db, tenant_id, Some(Uuid::new_v4()), create_command())
        .await
        .expect("generic Category definition should create on PostgreSQL");
    assert!(
        generation(&db).await > generation_before,
        "generic definition mutation must advance durable Flex generation"
    );

    let entity = AttachedEntityRef {
        tenant_id,
        entity_type: TAXONOMY_CATEGORY_ENTITY_TYPE,
        entity_id,
    };
    let prepared = prepare_generic_attached_values_update(
        &db,
        entity.clone(),
        schema(),
        "ar",
        Some(json!({"badge": "gold", "tagline": "مرحبا"})),
    )
    .await
    .expect("PostgreSQL Category values should prepare");
    persist_prepared_generic_attached_values(&db, entity.clone(), &prepared)
        .await
        .expect("PostgreSQL Category values should persist");

    let resolved = resolve_generic_attached_values(&db, entity, schema(), "ar", "en")
        .await
        .expect("PostgreSQL Category values should resolve")
        .expect("resolved Category values should exist");
    assert_eq!(resolved, json!({"badge": "gold", "tagline": "مرحبا"}));

    let foreign = resolve_generic_attached_values(
        &db,
        AttachedEntityRef {
            tenant_id: Uuid::new_v4(),
            entity_type: TAXONOMY_CATEGORY_ENTITY_TYPE,
            entity_id,
        },
        schema(),
        "ar",
        "en",
    )
    .await
    .expect("foreign tenant lookup should succeed");
    assert!(
        foreign.is_none(),
        "generic values must remain tenant-isolated"
    );
}
