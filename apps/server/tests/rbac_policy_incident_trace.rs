use std::time::Duration;

use chrono::Utc;
use rustok_api::Permission;
use rustok_core::{UserRole, UserStatus};
use rustok_migrations::SqliteTestMigrator as Migrator;
use rustok_rbac::RbacRoleAssignmentDbWriter;
use rustok_server::common::settings::RustokSettings;
use rustok_server::models::_entities::user_roles;
use rustok_server::models::{tenants, users};
use rustok_server::services::rbac_invalidation_generation::start_rbac_invalidation_generation_watchdog;
use rustok_server::services::rbac_service::RbacService;
use rustok_server::services::server_runtime_context::ServerRuntimeContext;
use rustok_telemetry::rbac_invalidation_metrics::{
    RBAC_INVALIDATION_APPLIED_GENERATION, RBAC_INVALIDATION_FULL_CLEARS_TOTAL,
    RBAC_INVALIDATION_RECOVERIES_TOTAL,
};
use rustok_test_utils::db::setup_test_db_with_migrations;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use serial_test::serial;
use uuid::Uuid;

#[derive(Debug)]
struct RbacPolicyIncidentPacket {
    incident_id: Uuid,
    tenant_id: Uuid,
    user_id: Uuid,
    evaluator_allowed_before_recovery: bool,
    relation_assigned_role_count: usize,
    relation_permission_count: usize,
    relation_grants_required_permission: bool,
    cache_hit: bool,
    cache_permissions_count: usize,
    durable_generation: u64,
    applied_generation_before: u64,
    recovery_action: &'static str,
    recovery_count_delta: u64,
    full_clear_count_delta: u64,
    applied_generation_after: u64,
    evaluator_allowed_after_recovery: bool,
}

async fn insert_tenant_and_user(db: &sea_orm::DatabaseConnection) -> (Uuid, Uuid) {
    let tenant_id = rustok_core::generate_id();
    let user_id = rustok_core::generate_id();

    tenants::Entity::insert(tenants::ActiveModel {
        id: Set(tenant_id),
        name: Set("RBAC policy incident tenant".to_string()),
        slug: Set(format!("rbac-policy-incident-{tenant_id}")),
        domain: Set(None),
        settings: Set(serde_json::json!({})),
        default_locale: Set("en".to_string()),
        is_active: Set(true),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    })
    .exec(db)
    .await
    .expect("insert incident tenant");

    users::Entity::insert(users::ActiveModel {
        id: Set(user_id),
        tenant_id: Set(tenant_id),
        email: Set(format!("rbac-policy-incident-{user_id}@example.com")),
        password_hash: Set("hash".to_string()),
        name: Set(None),
        status: Set(UserStatus::Active),
        email_verified_at: Set(None),
        last_login_at: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    })
    .exec(db)
    .await
    .expect("insert incident user");

    (tenant_id, user_id)
}

async fn wait_for_applied_generation(expected: u64) {
    tokio::time::timeout(Duration::from_secs(7), async {
        loop {
            if RBAC_INVALIDATION_APPLIED_GENERATION.get() == expected as i64 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("durable RBAC generation was not applied before timeout");
}

#[tokio::test]
#[serial]
async fn missed_publication_incident_connects_decision_relations_cache_generation_and_recovery() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let (tenant_id, user_id) = insert_tenant_and_user(&db).await;
    let required_permission = Permission::SETTINGS_MANAGE;

    let writer = RbacRoleAssignmentDbWriter::new(db.clone());
    writer
        .assign_role_permissions(tenant_id, user_id, UserRole::Customer)
        .await
        .expect("seed canonical tenant roles");
    RbacService::replace_user_role_committed(&db, &user_id, &tenant_id, UserRole::Admin)
        .await
        .expect("commit initial admin role and durable generation");

    let initial_generation = rustok_rbac::read_permission_invalidation_generation(&db)
        .await
        .expect("read initial durable generation");
    assert!(initial_generation > 0);

    let context = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    start_rbac_invalidation_generation_watchdog(&context)
        .await
        .expect("start durable generation watchdog");
    wait_for_applied_generation(initial_generation).await;

    RbacService::invalidate_user_rbac_caches(&tenant_id, &user_id).await;
    assert!(
        RbacService::has_permission(&db, &tenant_id, &user_id, &required_permission)
            .await
            .expect("warm authoritative permission snapshot")
    );

    let transaction = db.begin().await.expect("begin missed-publication mutation");
    user_roles::Entity::delete_many()
        .filter(user_roles::Column::UserId.eq(user_id))
        .exec(&transaction)
        .await
        .expect("remove role relation inside owner transaction");
    let durable_generation = rustok_rbac::reserve_permission_invalidation_generation(&transaction)
        .await
        .expect("reserve durable invalidation generation");
    transaction
        .commit()
        .await
        .expect("commit relation mutation and durable generation");

    let metrics_before_stale_decision = RbacService::metrics_snapshot();
    let evaluator_allowed_before_recovery =
        RbacService::has_permission(&db, &tenant_id, &user_id, &required_permission)
            .await
            .expect("evaluate stale cached permission before watchdog recovery");
    let metrics_after_stale_decision = RbacService::metrics_snapshot();
    let cached_permissions = RbacService::get_user_permissions(&db, &tenant_id, &user_id)
        .await
        .expect("read stale cached permission snapshot");
    let assigned_roles = user_roles::Entity::find()
        .filter(user_roles::Column::UserId.eq(user_id))
        .all(&db)
        .await
        .expect("read incident relation state");
    let authoritative_permissions =
        RbacService::get_user_permissions_authoritative(&db, &tenant_id, &user_id)
            .await
            .expect("read authoritative incident relation permissions");

    let recovery_before = RBAC_INVALIDATION_RECOVERIES_TOTAL
        .with_label_values(&["generation_advanced"])
        .get();
    let full_clear_before = RBAC_INVALIDATION_FULL_CLEARS_TOTAL
        .with_label_values(&["generation_advanced"])
        .get();
    let applied_generation_before = RBAC_INVALIDATION_APPLIED_GENERATION.get() as u64;

    assert!(evaluator_allowed_before_recovery);
    assert!(assigned_roles.is_empty());
    assert!(!authoritative_permissions.contains(&required_permission));
    assert!(cached_permissions.contains(&required_permission));
    assert_eq!(
        metrics_after_stale_decision.permission_cache_hits,
        metrics_before_stale_decision.permission_cache_hits + 1
    );
    assert!(durable_generation > applied_generation_before);

    wait_for_applied_generation(durable_generation).await;

    let evaluator_allowed_after_recovery =
        RbacService::has_permission(&db, &tenant_id, &user_id, &required_permission)
            .await
            .expect("evaluate permission after durable watchdog recovery");
    let recovery_after = RBAC_INVALIDATION_RECOVERIES_TOTAL
        .with_label_values(&["generation_advanced"])
        .get();
    let full_clear_after = RBAC_INVALIDATION_FULL_CLEARS_TOTAL
        .with_label_values(&["generation_advanced"])
        .get();
    let applied_generation_after = RBAC_INVALIDATION_APPLIED_GENERATION.get() as u64;

    let packet = RbacPolicyIncidentPacket {
        incident_id: Uuid::new_v4(),
        tenant_id,
        user_id,
        evaluator_allowed_before_recovery,
        relation_assigned_role_count: assigned_roles.len(),
        relation_permission_count: authoritative_permissions.len(),
        relation_grants_required_permission: authoritative_permissions
            .contains(&required_permission),
        cache_hit: metrics_after_stale_decision.permission_cache_hits
            == metrics_before_stale_decision.permission_cache_hits + 1,
        cache_permissions_count: cached_permissions.len(),
        durable_generation,
        applied_generation_before,
        recovery_action: "generation_advanced_full_clear",
        recovery_count_delta: recovery_after.saturating_sub(recovery_before),
        full_clear_count_delta: full_clear_after.saturating_sub(full_clear_before),
        applied_generation_after,
        evaluator_allowed_after_recovery,
    };

    tracing::info!(
        incident_id = %packet.incident_id,
        tenant_id = %packet.tenant_id,
        user_id = %packet.user_id,
        required_permission = %required_permission,
        evaluator_allowed_before_recovery = packet.evaluator_allowed_before_recovery,
        relation_assigned_role_count = packet.relation_assigned_role_count,
        relation_permission_count = packet.relation_permission_count,
        relation_grants_required_permission = packet.relation_grants_required_permission,
        cache_hit = packet.cache_hit,
        cache_permissions_count = packet.cache_permissions_count,
        durable_generation = packet.durable_generation,
        applied_generation_before = packet.applied_generation_before,
        recovery_action = packet.recovery_action,
        recovery_count_delta = packet.recovery_count_delta,
        full_clear_count_delta = packet.full_clear_count_delta,
        applied_generation_after = packet.applied_generation_after,
        evaluator_allowed_after_recovery = packet.evaluator_allowed_after_recovery,
        "rbac policy incident packet"
    );
    println!("rbac policy incident packet: {packet:?}");

    assert_eq!(packet.relation_assigned_role_count, 0);
    assert!(!packet.relation_grants_required_permission);
    assert!(packet.cache_hit);
    assert!(packet.cache_permissions_count > packet.relation_permission_count);
    assert_eq!(packet.recovery_count_delta, 1);
    assert_eq!(packet.full_clear_count_delta, 1);
    assert_eq!(packet.applied_generation_after, packet.durable_generation);
    assert!(!packet.evaluator_allowed_after_recovery);
}
