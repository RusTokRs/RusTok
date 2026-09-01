#![cfg(feature = "mod-groups")]

use std::sync::Arc;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use rustok_api::{PortActor, PortContext};
use rustok_groups::{
    GroupAccessReadPort, GroupCommandPort, GroupMembershipEffectiveStatus,
    GroupMembershipEnforcementCommandPort, GroupMembershipEnforcementCommandService,
    GroupMembershipEnforcementReadPort, GroupMembershipEnforcementService, GroupsService,
    ReadGroupMembershipEnforcementRequest, SetGroupFeatureRequest, SuspendGroupMembershipRequest,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use tokio::sync::Barrier;
use uuid::Uuid;

const POSTGRES_URL_ENV: &str = "RUSTOK_GROUPS_TEST_POSTGRES_URL";
const ROUNDS: usize = 8;

fn schema_url(base: &str, schema: &str) -> String {
    let separator = if base.contains('?') { '&' } else { '?' };
    format!("{base}{separator}options=-csearch_path%3D{schema}%2Cpublic")
}

async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .max_connections(1)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("Groups feature enforcement PostgreSQL connection should open")
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply for PostgreSQL feature evidence");
    }
}

async fn seed_group_fixture(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
    admin_id: Uuid,
    handle: &str,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, visibility, status, member_count)
VALUES ('{group_id}', '{tenant_id}', '{owner_id}', '{handle}', 'public', 'active', 2);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{tenant_id}', '{group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{admin_id}', 'admin', 'active', CURRENT_TIMESTAMP);
"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("Groups feature enforcement PostgreSQL fixture should seed");
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, operation: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("groups-feature-postgres-{operation}-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
    .with_idempotency_key(format!("postgres-{operation}-{}", Uuid::new_v4()))
}

fn read_context(tenant_id: Uuid, actor_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("groups-feature-postgres-read-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
}

fn enforcement_read_context(tenant_id: Uuid, owner_id: Uuid) -> PortContext {
    read_context(tenant_id, owner_id).with_claim("groups:access:read")
}

fn feature_request(group_id: Uuid, phase: &str) -> SetGroupFeatureRequest {
    SetGroupFeatureRequest {
        group_id,
        feature_key: "forum.discussions".to_string(),
        contract_version: "groups-feature-evidence.v1".to_string(),
        enabled: true,
        sort_order: 10,
        configuration: serde_json::json!({ "phase": phase }),
    }
}

async fn group_version(db: &DatabaseConnection, tenant_id: Uuid, group_id: Uuid) -> i64 {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT version FROM groups WHERE tenant_id = '{tenant_id}' AND id = '{group_id}'"
            ),
        ))
        .await
        .expect("PostgreSQL group version query should succeed")
        .expect("group should exist");
    row.try_get("", "version")
        .expect("group version should decode")
}

async fn member_count(db: &DatabaseConnection, tenant_id: Uuid, group_id: Uuid) -> i64 {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT member_count FROM groups WHERE tenant_id = '{tenant_id}' AND id = '{group_id}'"
            ),
        ))
        .await
        .expect("PostgreSQL group member_count query should succeed")
        .expect("group should exist");
    row.try_get("", "member_count")
        .expect("group member_count should decode")
}

async fn membership_snapshot(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> (String, i64) {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT status, revision FROM group_memberships WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{user_id}'"
            ),
        ))
        .await
        .expect("PostgreSQL membership snapshot query should succeed")
        .expect("membership should exist");
    (
        row.try_get("", "status")
            .expect("membership status should decode"),
        row.try_get("", "revision")
            .expect("membership revision should decode"),
    )
}

async fn feature_phase(
    db: DatabaseConnection,
    tenant_id: Uuid,
    owner_id: Uuid,
    group_id: Uuid,
) -> Option<String> {
    let service = GroupsService::new(db);
    GroupAccessReadPort::enabled_group_features(
        &service,
        read_context(tenant_id, owner_id),
        group_id,
    )
    .await
    .expect("owner should read enabled PostgreSQL group features")
    .into_iter()
    .find(|feature| feature.feature_key == "forum.discussions")
    .and_then(|feature| {
        feature
            .configuration
            .get("phase")
            .and_then(|value| value.as_str())
            .map(str::to_owned)
    })
}

#[tokio::test]
#[ignore = "requires RUSTOK_GROUPS_TEST_POSTGRES_URL"]
async fn feature_settings_follow_suspension_expiry_and_serialization_postgres() {
    let base_url = std::env::var(POSTGRES_URL_ENV)
        .expect("RUSTOK_GROUPS_TEST_POSTGRES_URL must be configured");
    let schema_name = format!("groups_feature_enforcement_{}", Uuid::new_v4().simple());
    let admin_db = connect(&base_url).await;
    admin_db
        .execute_unprepared(&format!("CREATE SCHEMA {schema_name}"))
        .await
        .expect("isolated Groups feature enforcement schema should create");
    let scoped_url = schema_url(&base_url, &schema_name);
    let db = connect(&scoped_url).await;
    install_groups_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    seed_group_fixture(
        &db,
        tenant_id,
        group_id,
        owner_id,
        admin_id,
        "feature-postgres-expiry",
    )
    .await;

    let base_version = group_version(&db, tenant_id, group_id).await;
    let feature_service = GroupsService::new(db.clone());
    GroupCommandPort::set_group_feature(
        &feature_service,
        write_context(tenant_id, admin_id, "initial-feature"),
        feature_request(group_id, "before-suspension"),
    )
    .await
    .expect("effective-active PostgreSQL admin should configure the group feature");
    let feature_version = group_version(&db, tenant_id, group_id).await;
    assert_eq!(feature_version, base_version + 1);

    let expires_at = Utc::now() + ChronoDuration::seconds(2);
    let enforcement = GroupMembershipEnforcementCommandService::new(db.clone());
    let suspended = GroupMembershipEnforcementCommandPort::suspend_membership(
        &enforcement,
        write_context(tenant_id, owner_id, "suspend-feature-admin"),
        SuspendGroupMembershipRequest {
            group_id,
            target_user_id: admin_id,
            expected_membership_revision: 1,
            reason_code: "harassment".to_string(),
            effective_until: Some(expires_at),
        },
    )
    .await
    .expect("owner should suspend PostgreSQL feature administrator");
    assert_eq!(suspended.group_version as i64, feature_version + 1);
    assert_eq!(suspended.member_count, 2);
    assert_eq!(
        membership_snapshot(&db, tenant_id, group_id, admin_id).await,
        ("active".to_string(), 2)
    );

    let blocked = GroupCommandPort::set_group_feature(
        &feature_service,
        write_context(tenant_id, admin_id, "blocked-feature"),
        feature_request(group_id, "must-not-commit"),
    )
    .await
    .expect_err("effective-suspended PostgreSQL admin must not configure a feature");
    assert_eq!(blocked.code, "groups.membership_suspended");
    assert!(!blocked.retryable);
    assert_eq!(
        group_version(&db, tenant_id, group_id).await,
        suspended.group_version as i64
    );
    assert_eq!(
        feature_phase(db.clone(), tenant_id, owner_id, group_id)
            .await
            .as_deref(),
        Some("before-suspension")
    );

    let effective_reader = GroupMembershipEnforcementService::new(db.clone());
    let during = GroupMembershipEnforcementReadPort::read_membership_enforcement(
        &effective_reader,
        enforcement_read_context(tenant_id, owner_id),
        ReadGroupMembershipEnforcementRequest {
            group_id,
            user_id: admin_id,
        },
    )
    .await
    .expect("owner should read suspended PostgreSQL feature administrator state");
    assert_eq!(
        during.effective_status,
        GroupMembershipEffectiveStatus::Suspended
    );

    let remaining = expires_at
        .signed_duration_since(Utc::now())
        .to_std()
        .unwrap_or_default();
    tokio::time::sleep(remaining + Duration::from_millis(100)).await;

    let after_expiry = GroupMembershipEnforcementReadPort::read_membership_enforcement(
        &effective_reader,
        enforcement_read_context(tenant_id, owner_id),
        ReadGroupMembershipEnforcementRequest {
            group_id,
            user_id: admin_id,
        },
    )
    .await
    .expect("owner-clock expiry should restore PostgreSQL feature authority");
    assert_eq!(
        after_expiry.effective_status,
        GroupMembershipEffectiveStatus::Active
    );
    assert_eq!(after_expiry.membership_revision, Some(2));

    GroupCommandPort::set_group_feature(
        &feature_service,
        write_context(tenant_id, admin_id, "restored-feature"),
        feature_request(group_id, "after-expiry"),
    )
    .await
    .expect("expired PostgreSQL suspension should restore feature authority without cleanup");
    assert_eq!(
        group_version(&db, tenant_id, group_id).await,
        suspended.group_version as i64 + 1
    );
    assert_eq!(
        membership_snapshot(&db, tenant_id, group_id, admin_id).await,
        ("active".to_string(), 2)
    );
    assert_eq!(member_count(&db, tenant_id, group_id).await, 2);
    assert_eq!(
        feature_phase(db.clone(), tenant_id, owner_id, group_id)
            .await
            .as_deref(),
        Some("after-expiry")
    );

    drop(effective_reader);
    drop(enforcement);
    drop(feature_service);

    for round in 0..ROUNDS {
        let tenant_id = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let admin_id = Uuid::new_v4();
        seed_group_fixture(
            &db,
            tenant_id,
            group_id,
            owner_id,
            admin_id,
            &format!("feature-postgres-race-{round}"),
        )
        .await;
        let race_base_version = group_version(&db, tenant_id, group_id).await;

        let feature_db = connect(&scoped_url).await;
        let enforcement_db = connect(&scoped_url).await;
        let barrier = Arc::new(Barrier::new(3));

        let feature_barrier = barrier.clone();
        let feature_task = tokio::spawn(async move {
            let service = GroupsService::new(feature_db);
            feature_barrier.wait().await;
            GroupCommandPort::set_group_feature(
                &service,
                write_context(tenant_id, admin_id, "raced-feature"),
                feature_request(group_id, "raced"),
            )
            .await
        });

        let enforcement_barrier = barrier.clone();
        let enforcement_task = tokio::spawn(async move {
            let service = GroupMembershipEnforcementCommandService::new(enforcement_db);
            enforcement_barrier.wait().await;
            GroupMembershipEnforcementCommandPort::suspend_membership(
                &service,
                write_context(tenant_id, owner_id, "raced-suspension"),
                SuspendGroupMembershipRequest {
                    group_id,
                    target_user_id: admin_id,
                    expected_membership_revision: 1,
                    reason_code: "concurrent_settings_review".to_string(),
                    effective_until: None,
                },
            )
            .await
        });

        barrier.wait().await;
        let feature_result = feature_task
            .await
            .expect("PostgreSQL feature race task should join without panic");
        let suspension = enforcement_task
            .await
            .expect("PostgreSQL enforcement race task should join without panic")
            .expect("owner suspension must serialize to a successful PostgreSQL commit");
        assert_eq!(suspension.membership_revision, 2);
        assert_eq!(suspension.member_count, 2);

        let phase = feature_phase(db.clone(), tenant_id, owner_id, group_id).await;
        match feature_result {
            Ok(feature) => {
                assert_eq!(feature.feature_key, "forum.discussions");
                assert_eq!(phase.as_deref(), Some("raced"));
                assert_eq!(
                    suspension.group_version as i64,
                    race_base_version + 2,
                    "successful PostgreSQL feature write must serialize before suspension"
                );
            }
            Err(error) => {
                assert_eq!(error.code, "groups.membership_suspended");
                assert!(!error.retryable);
                assert_eq!(phase, None);
                assert_eq!(
                    suspension.group_version as i64,
                    race_base_version + 1,
                    "denied PostgreSQL feature write must not advance group version"
                );
            }
        }

        assert_eq!(
            group_version(&db, tenant_id, group_id).await,
            suspension.group_version as i64
        );
        assert_eq!(
            membership_snapshot(&db, tenant_id, group_id, admin_id).await,
            ("active".to_string(), 2)
        );
        assert_eq!(member_count(&db, tenant_id, group_id).await, 2);
    }

    drop(db);
    admin_db
        .execute_unprepared(&format!("DROP SCHEMA {schema_name} CASCADE"))
        .await
        .expect("isolated Groups feature enforcement schema should drop");
}
