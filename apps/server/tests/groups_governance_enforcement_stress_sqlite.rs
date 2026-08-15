#![cfg(feature = "mod-groups")]

use std::sync::Arc;
use std::time::Duration;

use rustok_api::{PortActor, PortContext};
use rustok_groups::{
    ChangeGroupRoleRequest, GroupGovernanceCommandPort, GroupGovernanceService,
    GroupMembershipEnforcementCommandPort, GroupMembershipEnforcementCommandService, GroupRole,
    SuspendGroupMembershipRequest,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use tempfile::TempDir;
use tokio::sync::Barrier;
use uuid::Uuid;

const ROUNDS: usize = 3;
const TARGETS_PER_ROUND: usize = 8;
const SQLITE_BUSY_TIMEOUT_MS: u64 = 5_000;
const PAIR_TIMEOUT: Duration = Duration::from_secs(30);

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp
        .path()
        .join("groups-governance-enforcement-stress.sqlite");
    format!("sqlite://{}?mode=rwc", path.display())
}

async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .max_connections(1)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("Groups governance stress SQLite connection should open");
    db.execute_unprepared(&format!("PRAGMA busy_timeout = {SQLITE_BUSY_TIMEOUT_MS};"))
        .await
        .expect("Groups governance stress SQLite connection should configure busy timeout");
    db
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply for governance stress evidence");
    }
}

async fn seed_group_fixture(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
    targets: &[Uuid],
    round: usize,
) {
    let mut memberships = format!(
        "('{}', '{tenant_id}', '{group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP)",
        Uuid::new_v4()
    );
    for target_id in targets {
        memberships.push_str(&format!(
            ",\n('{}', '{tenant_id}', '{group_id}', '{target_id}', 'member', 'active', CURRENT_TIMESTAMP)",
            Uuid::new_v4()
        ));
    }

    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES ('{group_id}', '{tenant_id}', '{owner_id}', 'governance-stress-{round}', {});

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    {memberships};
"#,
        targets.len() + 1,
    ))
    .await
    .expect("Groups governance stress SQLite fixture should seed");
}

fn write_context(tenant_id: Uuid, owner_id: Uuid, operation: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(owner_id.to_string()),
        "en",
        format!("groups-governance-stress-sqlite-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(20))
    .with_idempotency_key(format!("{operation}-{}", Uuid::new_v4()))
}

async fn group_version(db: &DatabaseConnection, tenant_id: Uuid, group_id: Uuid) -> i64 {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT version FROM groups WHERE tenant_id = '{tenant_id}' AND id = '{group_id}'"
            ),
        ))
        .await
        .expect("group version query should succeed")
        .expect("group should exist");
    row.try_get("", "version")
        .expect("group version should decode")
}

async fn group_member_count(db: &DatabaseConnection, tenant_id: Uuid, group_id: Uuid) -> i64 {
    let row = db
        .query_one(Statement::from_string(
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

async fn membership_revision(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> i64 {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT revision FROM group_memberships WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{user_id}'"
            ),
        ))
        .await
        .expect("membership revision query should succeed")
        .expect("membership should exist");
    row.try_get("", "revision")
        .expect("membership revision should decode")
}

async fn membership_role(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> String {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT role FROM group_memberships WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{user_id}'"
            ),
        ))
        .await
        .expect("membership role query should succeed")
        .expect("membership should exist");
    row.try_get("", "role")
        .expect("membership role should decode")
}

async fn active_enforcement_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> i64 {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT COUNT(*) AS count FROM group_membership_enforcements WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{user_id}' AND revoked_at IS NULL"
            ),
        ))
        .await
        .expect("enforcement count query should succeed")
        .expect("enforcement count should exist");
    row.try_get("", "count")
        .expect("enforcement count should decode")
}

#[tokio::test]
async fn governance_enforcement_fanout_contention_is_deadlock_free_sqlite() {
    let temp =
        tempfile::tempdir().expect("temporary Groups governance stress directory should create");
    let url = sqlite_fixture_url(&temp);
    let fixture_db = connect(&url).await;
    fixture_db
        .execute_unprepared("PRAGMA journal_mode = WAL;")
        .await
        .expect("Groups governance stress SQLite fixture should enable WAL");
    install_groups_schema(&fixture_db).await;

    for round in 0..ROUNDS {
        let tenant_id = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let targets = (0..TARGETS_PER_ROUND)
            .map(|_| Uuid::new_v4())
            .collect::<Vec<_>>();
        seed_group_fixture(&fixture_db, tenant_id, group_id, owner_id, &targets, round).await;

        let base_version = group_version(&fixture_db, tenant_id, group_id).await;
        assert_eq!(
            group_member_count(&fixture_db, tenant_id, group_id).await,
            (TARGETS_PER_ROUND + 1) as i64
        );
        for target_id in &targets {
            assert_eq!(
                membership_revision(&fixture_db, tenant_id, group_id, *target_id).await,
                1
            );
        }

        let barrier = Arc::new(Barrier::new(TARGETS_PER_ROUND * 2 + 1));
        let mut task_pairs = Vec::with_capacity(TARGETS_PER_ROUND);

        for (index, target_id) in targets.iter().copied().enumerate() {
            let role_db = connect(&url).await;
            let suspension_db = connect(&url).await;

            let role_barrier = barrier.clone();
            let role_task = tokio::spawn(async move {
                let service = GroupGovernanceService::new(role_db);
                role_barrier.wait().await;
                GroupGovernanceCommandPort::change_group_role(
                    &service,
                    write_context(
                        tenant_id,
                        owner_id,
                        &format!("round-{round}-target-{index}-role"),
                    ),
                    ChangeGroupRoleRequest {
                        group_id,
                        target_user_id: target_id,
                        role: GroupRole::Moderator,
                    },
                )
                .await
            });

            let suspension_barrier = barrier.clone();
            let suspension_task = tokio::spawn(async move {
                let service = GroupMembershipEnforcementCommandService::new(suspension_db);
                suspension_barrier.wait().await;
                GroupMembershipEnforcementCommandPort::suspend_membership(
                    &service,
                    write_context(
                        tenant_id,
                        owner_id,
                        &format!("round-{round}-target-{index}-suspend"),
                    ),
                    SuspendGroupMembershipRequest {
                        group_id,
                        target_user_id: target_id,
                        expected_membership_revision: 1,
                        reason_code: "stress_governance_review".to_string(),
                        effective_until: None,
                    },
                )
                .await
            });

            task_pairs.push((target_id, role_task, suspension_task));
        }

        barrier.wait().await;

        let mut role_wins = 0usize;
        let mut suspension_wins = 0usize;
        for (target_id, role_task, suspension_task) in task_pairs {
            let (role_join, suspension_join) = tokio::time::timeout(PAIR_TIMEOUT, async {
                tokio::join!(role_task, suspension_task)
            })
            .await
            .expect(
                "SQLite governance/enforcement stress pair must not deadlock or exceed timeout",
            );

            let role_result =
                role_join.expect("SQLite governance stress task should join without panic");
            let suspension_result =
                suspension_join.expect("SQLite enforcement stress task should join without panic");

            match (role_result, suspension_result) {
                (Ok(role), Err(error)) => {
                    role_wins += 1;
                    assert_eq!(role.target_user_id, target_id);
                    assert_eq!(role.current_role, GroupRole::Moderator);
                    assert_eq!(
                        error.code,
                        "groups.membership_enforcement_revision_conflict"
                    );
                    assert!(!error.retryable);
                }
                (Err(error), Ok(suspension)) => {
                    suspension_wins += 1;
                    assert_eq!(error.code, "groups.membership_suspended");
                    assert!(!error.retryable);
                    assert_eq!(suspension.user_id, target_id);
                    assert_eq!(suspension.membership_revision, 2);
                }
                (role_result, suspension_result) => panic!(
                    "SQLite governance/enforcement stress target must produce exactly one owner-domain commit: target={target_id}, role={role_result:?}, suspension={suspension_result:?}"
                ),
            }
        }

        assert_eq!(role_wins + suspension_wins, TARGETS_PER_ROUND);
        assert_eq!(
            group_version(&fixture_db, tenant_id, group_id).await,
            base_version + TARGETS_PER_ROUND as i64,
            "exactly one successful material owner mutation per target must advance group version"
        );
        assert_eq!(
            group_member_count(&fixture_db, tenant_id, group_id).await,
            (TARGETS_PER_ROUND + 1) as i64,
            "temporary enforcement and role changes must preserve lifecycle member_count"
        );

        for target_id in targets {
            assert_eq!(
                membership_revision(&fixture_db, tenant_id, group_id, target_id).await,
                2,
                "each stress target must have exactly one material membership revision advance"
            );
            let role = membership_role(&fixture_db, tenant_id, group_id, target_id).await;
            let enforcement_count =
                active_enforcement_count(&fixture_db, tenant_id, group_id, target_id).await;
            assert!(
                (role == "moderator" && enforcement_count == 0)
                    || (role == "member" && enforcement_count == 1),
                "final SQLite stress state must match exactly one serialized winner: target={target_id}, role={role}, enforcement_count={enforcement_count}"
            );
        }
    }

    drop(fixture_db);
    drop(temp);
}
