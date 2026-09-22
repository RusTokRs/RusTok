#![cfg(feature = "mod-groups")]

use std::time::Duration;

use chrono::Utc;
use rustok_api::{PortActor, PortContext};
use rustok_groups::{
    GroupMembershipEffectiveStatus, GroupMembershipEnforcementCommandPort,
    GroupMembershipEnforcementCommandService, GroupMembershipEnforcementReadPort,
    GroupMembershipEnforcementService, ReadGroupMembershipEnforcementRequest,
    RevokeGroupMembershipSuspensionRequest, SuspendGroupMembershipRequest,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use tempfile::TempDir;
use uuid::Uuid;

async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("SQLite Groups expiry evidence connection should open")
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply in SQLite expiry evidence database");
    }
}

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp.path().join("groups-enforcement-expiry.sqlite");
    format!("sqlite://{}?mode=rwc", path.display())
}

fn user_write_context(
    tenant_id: Uuid,
    user_id: Uuid,
    idempotency_key: impl Into<String>,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        format!("groups-expiry-evidence-write-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
    .with_idempotency_key(idempotency_key)
}

fn owner_read_context(tenant_id: Uuid, owner_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(owner_id.to_string()),
        "en",
        format!("groups-expiry-evidence-read-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(10))
    .with_claim("groups:access:read")
}

async fn seed_group_fixture(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
    expiring_user_id: Uuid,
    revoked_user_id: Uuid,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES ('{group_id}', '{tenant_id}', '{owner_id}', 'enforcement-expiry-evidence', 3);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{tenant_id}', '{group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{expiring_user_id}', 'member', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{revoked_user_id}', 'member', 'active', CURRENT_TIMESTAMP);
"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("Groups SQLite expiry evidence fixture should seed");
}

async fn membership_revision(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) -> i64 {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT revision FROM group_memberships WHERE tenant_id = '{tenant_id}' AND user_id = '{user_id}'"
            ),
        ))
        .await
        .expect("membership revision query should succeed")
        .expect("membership should exist");
    row.try_get("", "revision")
        .expect("membership revision should decode")
}

async fn group_member_count(db: &DatabaseConnection, tenant_id: Uuid, group_id: Uuid) -> i64 {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT member_count FROM groups WHERE tenant_id = '{tenant_id}' AND id = '{group_id}'"
            ),
        ))
        .await
        .expect("group member_count query should succeed")
        .expect("group should exist");
    row.try_get("", "member_count")
        .expect("group member_count should decode")
}

async fn enforcement_projection(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
) -> (i64, Option<String>, Option<String>, String) {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT revision, effective_until, revoked_at, source_kind FROM group_membership_enforcements WHERE tenant_id = '{tenant_id}' AND user_id = '{user_id}'"
            ),
        ))
        .await
        .expect("enforcement projection query should succeed")
        .expect("enforcement projection should remain stored");
    (
        row.try_get("", "revision")
            .expect("enforcement revision should decode"),
        row.try_get("", "effective_until")
            .expect("effective_until should decode"),
        row.try_get("", "revoked_at")
            .expect("revoked_at should decode"),
        row.try_get("", "source_kind")
            .expect("source_kind should decode"),
    )
}

async fn read_effective_state(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    owner_id: Uuid,
    group_id: Uuid,
    target_user_id: Uuid,
) -> rustok_groups::GroupMembershipEffectiveState {
    let service = GroupMembershipEnforcementService::new(db.clone());
    GroupMembershipEnforcementReadPort::read_membership_enforcement(
        &service,
        owner_read_context(tenant_id, owner_id),
        ReadGroupMembershipEnforcementRequest {
            group_id,
            user_id: target_user_id,
        },
    )
    .await
    .expect("owner read claim should resolve effective membership state")
}

#[tokio::test]
async fn sqlite_group_membership_enforcement_expiry_and_revoke_restore_without_cleanup() {
    let temp =
        tempfile::tempdir().expect("temporary SQLite expiry evidence directory should create");
    let url = sqlite_fixture_url(&temp);
    let fixture = connect(&url).await;
    install_groups_schema(&fixture).await;

    let tenant_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let expiring_user_id = Uuid::new_v4();
    let revoked_user_id = Uuid::new_v4();
    seed_group_fixture(
        &fixture,
        tenant_id,
        group_id,
        owner_id,
        expiring_user_id,
        revoked_user_id,
    )
    .await;
    assert_eq!(group_member_count(&fixture, tenant_id, group_id).await, 3);

    let expiring_initial_revision =
        membership_revision(&fixture, tenant_id, expiring_user_id).await;
    let expiring_until = Utc::now() + chrono::Duration::seconds(2);
    let expiring_command = GroupMembershipEnforcementCommandService::new(connect(&url).await);
    let expiring_suspension = GroupMembershipEnforcementCommandPort::suspend_membership(
        &expiring_command,
        user_write_context(
            tenant_id,
            owner_id,
            format!("expiry-suspend-{}", Uuid::new_v4()),
        ),
        SuspendGroupMembershipRequest {
            group_id,
            target_user_id: expiring_user_id,
            expected_membership_revision: expiring_initial_revision,
            reason_code: "temporary_review".to_string(),
            effective_until: Some(expiring_until),
        },
    )
    .await
    .expect("owner should create an expiring direct-local suspension");
    assert_eq!(
        expiring_suspension.effective_status,
        GroupMembershipEffectiveStatus::Suspended
    );
    assert_eq!(
        expiring_suspension.membership_revision,
        expiring_initial_revision + 1
    );
    assert_eq!(expiring_suspension.member_count, 3);
    assert_eq!(group_member_count(&fixture, tenant_id, group_id).await, 3);

    let during_expiry =
        read_effective_state(&fixture, tenant_id, owner_id, group_id, expiring_user_id).await;
    assert_eq!(
        during_expiry.effective_status,
        GroupMembershipEffectiveStatus::Suspended
    );
    assert!(!during_expiry.active_member);
    let during_projection = during_expiry
        .enforcement
        .as_ref()
        .expect("active suspension should expose the current owner projection");
    assert!(during_projection.is_effective);
    assert_eq!(during_projection.source_kind.as_str(), "direct_local");

    tokio::time::sleep(Duration::from_millis(2300)).await;

    let after_expiry =
        read_effective_state(&fixture, tenant_id, owner_id, group_id, expiring_user_id).await;
    assert_eq!(
        after_expiry.effective_status,
        GroupMembershipEffectiveStatus::Active
    );
    assert!(after_expiry.active_member);
    let expired_projection = after_expiry
        .enforcement
        .as_ref()
        .expect("expired suspension projection should remain stored for owner evidence");
    assert!(!expired_projection.is_effective);
    assert!(expired_projection.effective_until.is_some());
    assert!(expired_projection.revoked_at.is_none());
    assert_eq!(expired_projection.source_kind.as_str(), "direct_local");
    assert_eq!(
        membership_revision(&fixture, tenant_id, expiring_user_id).await,
        expiring_initial_revision + 1,
        "clock expiry must not require a cleanup write or synthetic membership revision bump"
    );
    assert_eq!(group_member_count(&fixture, tenant_id, group_id).await, 3);
    let (_, persisted_until, persisted_revoked_at, persisted_source) =
        enforcement_projection(&fixture, tenant_id, expiring_user_id).await;
    assert!(persisted_until.is_some());
    assert!(persisted_revoked_at.is_none());
    assert_eq!(persisted_source, "direct_local");

    let revoke_initial_revision = membership_revision(&fixture, tenant_id, revoked_user_id).await;
    let revoke_command = GroupMembershipEnforcementCommandService::new(connect(&url).await);
    let permanent_suspension = GroupMembershipEnforcementCommandPort::suspend_membership(
        &revoke_command,
        user_write_context(
            tenant_id,
            owner_id,
            format!("revoke-suspend-{}", Uuid::new_v4()),
        ),
        SuspendGroupMembershipRequest {
            group_id,
            target_user_id: revoked_user_id,
            expected_membership_revision: revoke_initial_revision,
            reason_code: "manual_review".to_string(),
            effective_until: None,
        },
    )
    .await
    .expect("owner should create a direct-local suspension for revoke evidence");
    assert_eq!(
        permanent_suspension.effective_status,
        GroupMembershipEffectiveStatus::Suspended
    );
    assert_eq!(group_member_count(&fixture, tenant_id, group_id).await, 3);

    let before_revoke_projection =
        enforcement_projection(&fixture, tenant_id, revoked_user_id).await;
    assert!(before_revoke_projection.2.is_none());
    assert_eq!(before_revoke_projection.3, "direct_local");

    let revoked = GroupMembershipEnforcementCommandPort::revoke_membership_suspension(
        &revoke_command,
        user_write_context(
            tenant_id,
            owner_id,
            format!("revoke-direct-{}", Uuid::new_v4()),
        ),
        RevokeGroupMembershipSuspensionRequest {
            group_id,
            target_user_id: revoked_user_id,
            expected_membership_revision: permanent_suspension.membership_revision,
            reason_code: "review_complete".to_string(),
        },
    )
    .await
    .expect("owner should revoke an active direct-local suspension");
    assert_eq!(
        revoked.effective_status,
        GroupMembershipEffectiveStatus::Active
    );
    assert!(revoked.revoked_at.is_some());
    assert_eq!(
        revoked.membership_revision,
        permanent_suspension.membership_revision + 1
    );
    assert_eq!(revoked.member_count, 3);
    assert_eq!(group_member_count(&fixture, tenant_id, group_id).await, 3);

    let after_revoke =
        read_effective_state(&fixture, tenant_id, owner_id, group_id, revoked_user_id).await;
    assert_eq!(
        after_revoke.effective_status,
        GroupMembershipEffectiveStatus::Active
    );
    assert!(after_revoke.active_member);
    let revoked_projection = after_revoke
        .enforcement
        .as_ref()
        .expect("revoked suspension projection should remain stored for owner evidence");
    assert!(!revoked_projection.is_effective);
    assert!(revoked_projection.revoked_at.is_some());
    assert_eq!(revoked_projection.source_kind.as_str(), "direct_local");

    let after_revoke_projection =
        enforcement_projection(&fixture, tenant_id, revoked_user_id).await;
    assert_eq!(
        after_revoke_projection.0,
        before_revoke_projection.0 + 1,
        "revoke should mutate the current projection in place rather than delete it"
    );
    assert!(after_revoke_projection.2.is_some());
    assert_eq!(after_revoke_projection.3, "direct_local");
    assert_eq!(
        membership_revision(&fixture, tenant_id, revoked_user_id).await,
        revoke_initial_revision + 2,
        "suspend and revoke should each advance the membership revision exactly once"
    );
    assert_eq!(group_member_count(&fixture, tenant_id, group_id).await, 3);

    drop(expiring_command);
    drop(revoke_command);
    drop(fixture);
    drop(temp);
}
