use std::collections::HashMap;

use flex::{
    AttachedEntityRef, load_exact_locale_values, load_generic_attached_shared_values,
    persist_prepared_generic_attached_values, prepare_generic_attached_values_update,
    resolve_generic_attached_values,
};
use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FieldType};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use serde_json::json;
use uuid::Uuid;

const ENTITY_TYPE: &str = "taxonomy.category";

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

async fn test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite Flex fixture should connect");
    db.execute_unprepared(
        "CREATE TABLE flex_attached_values (\
            id TEXT PRIMARY KEY NOT NULL, \
            tenant_id TEXT NOT NULL, \
            entity_type TEXT NOT NULL, \
            entity_id TEXT NOT NULL, \
            data TEXT NOT NULL, \
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
            UNIQUE (tenant_id, entity_type, entity_id)\
        )",
    )
    .await
    .expect("generic attached shared storage should create");
    db.execute_unprepared(
        "CREATE TABLE flex_attached_localized_values (\
            id TEXT PRIMARY KEY NOT NULL, \
            tenant_id TEXT NOT NULL, \
            entity_type TEXT NOT NULL, \
            entity_id TEXT NOT NULL, \
            field_key TEXT NOT NULL, \
            locale TEXT NOT NULL, \
            value TEXT NOT NULL, \
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
            UNIQUE (tenant_id, entity_type, entity_id, field_key, locale)\
        )",
    )
    .await
    .expect("generic attached localized storage should create");
    db
}

fn entity(tenant_id: Uuid, entity_id: Uuid) -> AttachedEntityRef<'static> {
    AttachedEntityRef {
        tenant_id,
        entity_type: ENTITY_TYPE,
        entity_id,
    }
}

#[tokio::test]
async fn generic_attached_values_split_shared_and_exact_locale_rows() {
    let db = test_db().await;
    let tenant_id = Uuid::new_v4();
    let entity_id = Uuid::new_v4();

    let prepared = prepare_generic_attached_values_update(
        &db,
        entity(tenant_id, entity_id),
        schema(),
        "ar",
        Some(json!({"badge": "gold", "tagline": "مرحبا"})),
    )
    .await
    .expect("Arabic write should prepare");
    persist_prepared_generic_attached_values(
        &db,
        entity(tenant_id, entity_id),
        &prepared,
    )
    .await
    .expect("Arabic write should persist");

    let shared = load_generic_attached_shared_values(&db, entity(tenant_id, entity_id))
        .await
        .expect("shared values should load");
    assert_eq!(shared, json!({"badge": "gold"}));

    let arabic = load_exact_locale_values(&db, tenant_id, ENTITY_TYPE, entity_id, "ar")
        .await
        .expect("Arabic values should load")
        .expect("Arabic row should exist");
    assert_eq!(arabic, json!({"tagline": "مرحبا"}));

    let resolved = resolve_generic_attached_values(
        &db,
        entity(tenant_id, entity_id),
        schema(),
        "ar",
        "en",
    )
    .await
    .expect("Arabic payload should resolve")
    .expect("resolved payload should exist");
    assert_eq!(resolved, json!({"badge": "gold", "tagline": "مرحبا"}));
}

#[tokio::test]
async fn exact_locale_authoring_does_not_seed_from_read_fallback() {
    let db = test_db().await;
    let tenant_id = Uuid::new_v4();
    let entity_id = Uuid::new_v4();

    let arabic = prepare_generic_attached_values_update(
        &db,
        entity(tenant_id, entity_id),
        schema(),
        "ar",
        Some(json!({"badge": "gold", "tagline": "مرحبا"})),
    )
    .await
    .expect("Arabic write should prepare");
    persist_prepared_generic_attached_values(&db, entity(tenant_id, entity_id), &arabic)
        .await
        .expect("Arabic write should persist");

    let english = prepare_generic_attached_values_update(
        &db,
        entity(tenant_id, entity_id),
        schema(),
        "en",
        Some(json!({"badge": "silver"})),
    )
    .await
    .expect("English write should prepare from exact English row only");
    persist_prepared_generic_attached_values(&db, entity(tenant_id, entity_id), &english)
        .await
        .expect("English write should persist");

    let english_exact = load_exact_locale_values(&db, tenant_id, ENTITY_TYPE, entity_id, "en")
        .await
        .expect("English exact lookup should succeed")
        .expect("localized schema persists an exact English row");
    assert_eq!(english_exact, json!({}));

    let arabic_exact = load_exact_locale_values(&db, tenant_id, ENTITY_TYPE, entity_id, "ar")
        .await
        .expect("Arabic exact lookup should succeed")
        .expect("Arabic row should remain");
    assert_eq!(arabic_exact, json!({"tagline": "مرحبا"}));

    let shared = load_generic_attached_shared_values(&db, entity(tenant_id, entity_id))
        .await
        .expect("shared values should load");
    assert_eq!(shared, json!({"badge": "silver"}));
}
