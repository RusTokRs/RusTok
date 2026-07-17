use std::time::Duration;

use flex::FieldDefinitionView;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use serde_json::json;
use uuid::Uuid;

use super::*;
use crate::common::settings::RustokSettings;

fn mock_view(field_key: &str) -> FieldDefinitionView {
    FieldDefinitionView {
        id: Uuid::new_v4(),
        field_key: field_key.to_string(),
        field_type: "text".to_string(),
        label: json!({"en": field_key}),
        description: None,
        is_localized: false,
        is_required: false,
        default_value: None,
        validation: None,
        position: 0,
        is_active: true,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    }
}

async fn sqlite_generation_db(generation: u64) -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite generation fixture should connect");
    restore_generation_table(&db, generation).await;
    db
}

async fn restore_generation_table(db: &DatabaseConnection, generation: u64) {
    db.execute_unprepared(&format!(
        "CREATE TABLE IF NOT EXISTS {FIELD_DEFINITION_CACHE_GENERATION_TABLE} (id INTEGER PRIMARY KEY, generation BIGINT NOT NULL, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)"
    ))
    .await
    .expect("Flex generation table should exist");
    db.execute_unprepared(&format!(
        "INSERT INTO {FIELD_DEFINITION_CACHE_GENERATION_TABLE} (id, generation) VALUES (1, {generation}) ON CONFLICT (id) DO UPDATE SET generation = excluded.generation, updated_at = CURRENT_TIMESTAMP"
    ))
    .await
    .expect("Flex generation singleton should be restored");
}

async fn set_generation(db: &DatabaseConnection, generation: u64) {
    db.execute_unprepared(&format!(
        "UPDATE {FIELD_DEFINITION_CACHE_GENERATION_TABLE} SET generation = {generation}, updated_at = CURRENT_TIMESTAMP WHERE id = 1"
    ))
    .await
    .expect("Flex generation should update");
}

async fn drop_generation_table(db: &DatabaseConnection) {
    db.execute_unprepared(&format!(
        "DROP TABLE {FIELD_DEFINITION_CACHE_GENERATION_TABLE}"
    ))
    .await
    .expect("Flex generation table should drop");
}

async fn seed_stale(cache: &FieldDefinitionCache, tenant_id: Uuid, marker: &str) {
    cache.set(tenant_id, "user", vec![mock_view(marker)]).await;
    assert!(cache.get(tenant_id, "user").await.is_some());
}

async fn wait_for_state(
    handle: &FieldDefinitionCacheGenerationReconciliationHandle,
    expected_ready: bool,
    expected_generation: Option<u64>,
) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let generation_matches = expected_generation
                .is_none_or(|generation| handle.applied_generation() == generation);
            if handle.is_ready() == expected_ready && generation_matches {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "Flex generation state did not become ready={expected_ready}, generation={expected_generation:?}; actual ready={}, generation={}",
            handle.is_ready(),
            handle.applied_generation()
        )
    });
}

async fn wait_for_empty(cache: &FieldDefinitionCache, tenant_id: Uuid) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while cache.get(tenant_id, "user").await.is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("Flex field-definition cache did not clear");
}

#[tokio::test]
async fn field_definition_cache_generation_recovers_two_replicas_across_faults() {
    let db = sqlite_generation_db(5).await;
    let tenant_id = Uuid::new_v4();
    let cache_a = FieldDefinitionCache::new();
    let cache_b = FieldDefinitionCache::new();
    seed_stale(&cache_a, tenant_id, "before-startup-a").await;
    seed_stale(&cache_b, tenant_id, "before-startup-b").await;

    let ctx_a = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let ctx_b = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    start_field_definition_cache_generation_reconciliation_with_timing(
        &ctx_a,
        cache_a.clone(),
        Duration::from_millis(20),
        Duration::from_millis(20),
    );
    start_field_definition_cache_generation_reconciliation_with_timing(
        &ctx_b,
        cache_b.clone(),
        Duration::from_millis(20),
        Duration::from_millis(20),
    );
    let handle_a = ctx_a
        .shared_get::<FieldDefinitionCacheGenerationReconciliationHandle>()
        .expect("first Flex generation handle");
    let handle_b = ctx_b
        .shared_get::<FieldDefinitionCacheGenerationReconciliationHandle>()
        .expect("second Flex generation handle");

    wait_for_state(&handle_a, true, Some(5)).await;
    wait_for_state(&handle_b, true, Some(5)).await;
    wait_for_empty(&cache_a, tenant_id).await;
    wait_for_empty(&cache_b, tenant_id).await;

    seed_stale(&cache_a, tenant_id, "before-advance-a").await;
    seed_stale(&cache_b, tenant_id, "before-advance-b").await;
    set_generation(&db, 6).await;
    wait_for_state(&handle_a, true, Some(6)).await;
    wait_for_state(&handle_b, true, Some(6)).await;
    wait_for_empty(&cache_a, tenant_id).await;
    wait_for_empty(&cache_b, tenant_id).await;

    seed_stale(&cache_a, tenant_id, "before-outage-a").await;
    seed_stale(&cache_b, tenant_id, "before-outage-b").await;
    drop_generation_table(&db).await;
    wait_for_state(&handle_a, false, Some(6)).await;
    wait_for_state(&handle_b, false, Some(6)).await;
    wait_for_empty(&cache_a, tenant_id).await;
    wait_for_empty(&cache_b, tenant_id).await;

    seed_stale(&cache_a, tenant_id, "during-outage-a").await;
    seed_stale(&cache_b, tenant_id, "during-outage-b").await;
    restore_generation_table(&db, 7).await;
    wait_for_state(&handle_a, true, Some(7)).await;
    wait_for_state(&handle_b, true, Some(7)).await;
    wait_for_empty(&cache_a, tenant_id).await;
    wait_for_empty(&cache_b, tenant_id).await;

    seed_stale(&cache_a, tenant_id, "before-regression-a").await;
    seed_stale(&cache_b, tenant_id, "before-regression-b").await;
    set_generation(&db, 3).await;
    wait_for_state(&handle_a, false, Some(7)).await;
    wait_for_state(&handle_b, false, Some(7)).await;
    wait_for_empty(&cache_a, tenant_id).await;
    wait_for_empty(&cache_b, tenant_id).await;

    seed_stale(&cache_a, tenant_id, "during-regression-a").await;
    seed_stale(&cache_b, tenant_id, "during-regression-b").await;
    set_generation(&db, 8).await;
    wait_for_state(&handle_a, true, Some(8)).await;
    wait_for_state(&handle_b, true, Some(8)).await;
    wait_for_empty(&cache_a, tenant_id).await;
    wait_for_empty(&cache_b, tenant_id).await;

    assert!(handle_a.is_running());
    assert!(handle_b.is_running());
    handle_a.abort();
    handle_b.abort();
}
