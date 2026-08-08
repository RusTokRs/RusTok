#![cfg(feature = "mod-groups")]

use std::time::Duration;

use async_graphql::{EmptySubscription, Request, Response, Schema};
use rustok_api::{
    AuthContext, HostRuntimeContext, PortActor, PortContext, PortErrorKind, TenantContext,
};
use rustok_groups::graphql_application_cas::{GroupsMutationRoot, GroupsQueryRoot};
use rustok_groups::{
    GroupMembershipEnforcementCommandPort, GroupMembershipEnforcementCommandService,
    GroupMembershipEnforcementMutationResult, RevokeGroupMembershipSuspensionRequest,
    SuspendGroupMembershipRequest,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
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
        .expect("Groups enforcement GraphQL parity SQLite connection should open")
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply for enforcement GraphQL parity evidence");
    }
}

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp
        .path()
        .join("groups-membership-enforcement-graphql-parity.sqlite");
    format!("sqlite://{}?mode=rwc", path.display())
}

async fn seed_group_fixture(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    native_group_id: Uuid,
    graphql_group_id: Uuid,
    owner_id: Uuid,
    target_id: Uuid,
    native_target_membership_id: Uuid,
    graphql_target_membership_id: Uuid,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES
    ('{native_group_id}', '{tenant_id}', '{owner_id}', 'enforcement-native-parity', 2),
    ('{graphql_group_id}', '{tenant_id}', '{owner_id}', 'enforcement-graphql-parity', 2);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{tenant_id}', '{native_group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{native_target_membership_id}', '{tenant_id}', '{native_group_id}', '{target_id}', 'member', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{graphql_group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{graphql_target_membership_id}', '{tenant_id}', '{graphql_group_id}', '{target_id}', 'member', 'active', CURRENT_TIMESTAMP);
"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("Groups enforcement GraphQL parity fixture should seed");
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Groups enforcement GraphQL parity".to_string(),
        slug: "groups-enforcement-graphql-parity".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}

fn auth_context(tenant_id: Uuid, owner_id: Uuid) -> AuthContext {
    AuthContext {
        user_id: owner_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: Vec::new(),
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    }
}

fn native_write_context(
    tenant_id: Uuid,
    owner_id: Uuid,
    idempotency_key: &str,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(owner_id.to_string()),
        "en",
        format!("groups-enforcement-native-parity-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(5))
    .with_idempotency_key(idempotency_key)
}

fn graphql_schema(
    db: DatabaseConnection,
) -> Schema<GroupsQueryRoot, GroupsMutationRoot, EmptySubscription> {
    Schema::build(
        GroupsQueryRoot::default(),
        GroupsMutationRoot::default(),
        EmptySubscription,
    )
    .data(HostRuntimeContext::new(db))
    .finish()
}

async fn execute_graphql(
    schema: &Schema<GroupsQueryRoot, GroupsMutationRoot, EmptySubscription>,
    tenant_id: Uuid,
    owner_id: Uuid,
    document: String,
) -> Response {
    schema
        .execute(
            Request::new(document)
                .data(tenant_context(tenant_id))
                .data(auth_context(tenant_id, owner_id)),
        )
        .await
}

fn response_json(response: Response) -> serde_json::Value {
    assert!(
        response.errors.is_empty(),
        "Groups enforcement GraphQL parity request should succeed: {:?}",
        response.errors
    );
    response
        .data
        .into_json()
        .expect("Groups enforcement GraphQL parity data should convert to JSON")
}

fn extension_json(error: &async_graphql::ServerError, key: &str) -> Option<serde_json::Value> {
    error
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get(key))
        .cloned()
        .and_then(|value| value.into_json().ok())
}

fn assert_graphql_result(
    graphql: &serde_json::Value,
    native: &GroupMembershipEnforcementMutationResult,
    expected_group_id: Uuid,
    expected_membership_id: Uuid,
    expected_replayed: bool,
) {
    assert_eq!(
        graphql["groupId"].as_str().map(str::to_owned),
        Some(expected_group_id.to_string())
    );
    assert_eq!(
        graphql["membershipId"].as_str().map(str::to_owned),
        Some(expected_membership_id.to_string())
    );
    assert_eq!(
        graphql["userId"].as_str().map(str::to_owned),
        Some(native.user_id.to_string())
    );
    assert_eq!(
        graphql["membershipRevision"].as_i64(),
        Some(native.membership_revision)
    );
    assert_eq!(graphql["groupVersion"].as_i64(), Some(native.group_version));
    assert_eq!(graphql["memberCount"].as_i64(), Some(native.member_count));
    assert_eq!(
        graphql["effectiveStatus"].as_str(),
        Some(native.effective_status.as_str())
    );
    assert_eq!(
        graphql["enforcementRevision"].as_i64(),
        Some(native.enforcement_revision)
    );
    assert_eq!(
        graphql["effectiveUntil"].is_null(),
        native.effective_until.is_none()
    );
    assert_eq!(graphql["revokedAt"].is_null(), native.revoked_at.is_none());
    assert_eq!(graphql["replayed"].as_bool(), Some(expected_replayed));
}

async fn owner_snapshot(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    target_id: Uuid,
) -> (i64, i64, String, String, i64, i64, String, i64) {
    let group = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT version, member_count FROM groups WHERE tenant_id = '{tenant_id}' AND id = '{group_id}'"
            ),
        ))
        .await
        .expect("group snapshot query should succeed")
        .expect("group should exist");
    let membership = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT role, status, revision FROM group_memberships WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{target_id}'"
            ),
        ))
        .await
        .expect("membership snapshot query should succeed")
        .expect("target membership should exist");
    let enforcement = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT revision, source_kind, CASE WHEN revoked_at IS NULL THEN 0 ELSE 1 END AS revoked FROM group_membership_enforcements WHERE tenant_id = '{tenant_id}' AND group_id = '{group_id}' AND user_id = '{target_id}'"
            ),
        ))
        .await
        .expect("enforcement snapshot query should succeed")
        .expect("enforcement row should exist");

    (
        group.try_get("", "version").expect("group version should decode"),
        group
            .try_get("", "member_count")
            .expect("member_count should decode"),
        membership.try_get("", "role").expect("role should decode"),
        membership
            .try_get("", "status")
            .expect("status should decode"),
        membership
            .try_get("", "revision")
            .expect("membership revision should decode"),
        enforcement
            .try_get("", "revision")
            .expect("enforcement revision should decode"),
        enforcement
            .try_get("", "source_kind")
            .expect("enforcement source should decode"),
        enforcement
            .try_get("", "revoked")
            .expect("revoked marker should decode"),
    )
}

#[tokio::test]
async fn direct_enforcement_native_and_final_graphql_share_owner_semantics_sqlite() {
    let temp = tempfile::tempdir()
        .expect("temporary Groups enforcement GraphQL parity directory should create");
    let url = sqlite_fixture_url(&temp);
    let db = connect(&url).await;
    install_groups_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let native_group_id = Uuid::new_v4();
    let graphql_group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    let native_target_membership_id = Uuid::new_v4();
    let graphql_target_membership_id = Uuid::new_v4();
    seed_group_fixture(
        &db,
        tenant_id,
        native_group_id,
        graphql_group_id,
        owner_id,
        target_id,
        native_target_membership_id,
        graphql_target_membership_id,
    )
    .await;

    let native = GroupMembershipEnforcementCommandService::new(db.clone());
    let schema = graphql_schema(db.clone());

    let native_suspend = GroupMembershipEnforcementCommandPort::suspend_membership(
        &native,
        native_write_context(tenant_id, owner_id, "native-suspend"),
        SuspendGroupMembershipRequest {
            group_id: native_group_id,
            target_user_id: target_id,
            expected_membership_revision: 1,
            reason_code: "transport_parity".to_string(),
            effective_until: None,
        },
    )
    .await
    .expect("native direct suspension should succeed");
    assert_eq!(native_suspend.membership_id, native_target_membership_id);
    assert_eq!(native_suspend.membership_revision, 2);
    assert_eq!(native_suspend.member_count, 2);
    assert!(!native_suspend.replayed);

    let graphql_suspend = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            format!(
                r#"
mutation {{
  suspendGroupMembership(
    idempotencyKey: "graphql-suspend",
    groupId: "{graphql_group_id}",
    targetUserId: "{target_id}",
    expectedMembershipRevision: 1,
    reasonCode: "transport_parity"
  ) {{
    groupId membershipId userId membershipRevision groupVersion memberCount
    effectiveStatus enforcementRevision effectiveUntil revokedAt replayed
  }}
}}
"#
            ),
        )
        .await,
    );
    assert_graphql_result(
        &graphql_suspend["suspendGroupMembership"],
        &native_suspend,
        graphql_group_id,
        graphql_target_membership_id,
        false,
    );

    let native_suspend_replay = GroupMembershipEnforcementCommandPort::suspend_membership(
        &native,
        native_write_context(tenant_id, owner_id, "native-suspend"),
        SuspendGroupMembershipRequest {
            group_id: native_group_id,
            target_user_id: target_id,
            expected_membership_revision: 1,
            reason_code: "transport_parity".to_string(),
            effective_until: None,
        },
    )
    .await
    .expect("native suspension receipt should replay while currently suspended");
    assert!(native_suspend_replay.replayed);
    assert_eq!(native_suspend_replay.group_version, native_suspend.group_version);

    let graphql_suspend_replay = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            format!(
                r#"
mutation {{
  suspendGroupMembership(
    idempotencyKey: "graphql-suspend",
    groupId: "{graphql_group_id}",
    targetUserId: "{target_id}",
    expectedMembershipRevision: 1,
    reasonCode: "transport_parity"
  ) {{
    groupId membershipId userId membershipRevision groupVersion memberCount
    effectiveStatus enforcementRevision effectiveUntil revokedAt replayed
  }}
}}
"#
            ),
        )
        .await,
    );
    assert_graphql_result(
        &graphql_suspend_replay["suspendGroupMembership"],
        &native_suspend_replay,
        graphql_group_id,
        graphql_target_membership_id,
        true,
    );

    let native_stale = GroupMembershipEnforcementCommandPort::suspend_membership(
        &native,
        native_write_context(tenant_id, owner_id, "native-stale-suspend"),
        SuspendGroupMembershipRequest {
            group_id: native_group_id,
            target_user_id: target_id,
            expected_membership_revision: 1,
            reason_code: "stale_transport_parity".to_string(),
            effective_until: None,
        },
    )
    .await
    .expect_err("fresh native stale suspension must fail revision CAS");
    assert_eq!(native_stale.kind, PortErrorKind::Conflict);
    assert_eq!(
        native_stale.code,
        "groups.membership_enforcement_revision_conflict"
    );
    assert!(!native_stale.retryable);

    let graphql_stale = execute_graphql(
        &schema,
        tenant_id,
        owner_id,
        format!(
            r#"
mutation {{
  suspendGroupMembership(
    idempotencyKey: "graphql-stale-suspend",
    groupId: "{graphql_group_id}",
    targetUserId: "{target_id}",
    expectedMembershipRevision: 1,
    reasonCode: "stale_transport_parity"
  ) {{ groupVersion }}
}}
"#
        ),
    )
    .await;
    assert_eq!(graphql_stale.errors.len(), 1);
    let graphql_stale_error = &graphql_stale.errors[0];
    assert_eq!(graphql_stale_error.message, native_stale.message);
    assert_eq!(
        extension_json(graphql_stale_error, "code")
            .and_then(|value| value.as_str().map(str::to_owned)),
        Some("BAD_USER_INPUT".to_string())
    );
    assert_eq!(
        extension_json(graphql_stale_error, "domainCode")
            .and_then(|value| value.as_str().map(str::to_owned)),
        Some("groups.membership_enforcement_revision_conflict".to_string())
    );
    assert_eq!(
        extension_json(graphql_stale_error, "retryable").and_then(|value| value.as_bool()),
        Some(false)
    );

    let native_revoke = GroupMembershipEnforcementCommandPort::revoke_membership_suspension(
        &native,
        native_write_context(tenant_id, owner_id, "native-revoke"),
        RevokeGroupMembershipSuspensionRequest {
            group_id: native_group_id,
            target_user_id: target_id,
            expected_membership_revision: 2,
            reason_code: "transport_parity_release".to_string(),
        },
    )
    .await
    .expect("native direct suspension revoke should succeed");
    assert_eq!(native_revoke.membership_revision, 3);
    assert_eq!(native_revoke.member_count, 2);
    assert!(native_revoke.revoked_at.is_some());
    assert_eq!(native_revoke.group_version, native_suspend.group_version + 1);
    assert!(!native_revoke.replayed);

    let graphql_revoke = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            format!(
                r#"
mutation {{
  revokeGroupMembershipSuspension(
    idempotencyKey: "graphql-revoke",
    groupId: "{graphql_group_id}",
    targetUserId: "{target_id}",
    expectedMembershipRevision: 2,
    reasonCode: "transport_parity_release"
  ) {{
    groupId membershipId userId membershipRevision groupVersion memberCount
    effectiveStatus enforcementRevision effectiveUntil revokedAt replayed
  }}
}}
"#
            ),
        )
        .await,
    );
    assert_graphql_result(
        &graphql_revoke["revokeGroupMembershipSuspension"],
        &native_revoke,
        graphql_group_id,
        graphql_target_membership_id,
        false,
    );

    let native_revoke_replay =
        GroupMembershipEnforcementCommandPort::revoke_membership_suspension(
            &native,
            native_write_context(tenant_id, owner_id, "native-revoke"),
            RevokeGroupMembershipSuspensionRequest {
                group_id: native_group_id,
                target_user_id: target_id,
                expected_membership_revision: 2,
                reason_code: "transport_parity_release".to_string(),
            },
        )
        .await
        .expect("native revoke receipt should replay after suspension is no longer active");
    assert!(native_revoke_replay.replayed);
    assert_eq!(native_revoke_replay.group_version, native_revoke.group_version);

    let graphql_revoke_replay = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            format!(
                r#"
mutation {{
  revokeGroupMembershipSuspension(
    idempotencyKey: "graphql-revoke",
    groupId: "{graphql_group_id}",
    targetUserId: "{target_id}",
    expectedMembershipRevision: 2,
    reasonCode: "transport_parity_release"
  ) {{
    groupId membershipId userId membershipRevision groupVersion memberCount
    effectiveStatus enforcementRevision effectiveUntil revokedAt replayed
  }}
}}
"#
            ),
        )
        .await,
    );
    assert_graphql_result(
        &graphql_revoke_replay["revokeGroupMembershipSuspension"],
        &native_revoke_replay,
        graphql_group_id,
        graphql_target_membership_id,
        true,
    );

    let native_suspend_after_revoke = GroupMembershipEnforcementCommandPort::suspend_membership(
        &native,
        native_write_context(tenant_id, owner_id, "native-suspend"),
        SuspendGroupMembershipRequest {
            group_id: native_group_id,
            target_user_id: target_id,
            expected_membership_revision: 1,
            reason_code: "transport_parity".to_string(),
            effective_until: None,
        },
    )
    .await
    .expect("old native suspension receipt must replay after later revoke state");
    assert!(native_suspend_after_revoke.replayed);
    assert_eq!(
        native_suspend_after_revoke.group_version,
        native_suspend.group_version
    );
    assert!(native_suspend_after_revoke.revoked_at.is_none());

    let graphql_suspend_after_revoke = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            format!(
                r#"
mutation {{
  suspendGroupMembership(
    idempotencyKey: "graphql-suspend",
    groupId: "{graphql_group_id}",
    targetUserId: "{target_id}",
    expectedMembershipRevision: 1,
    reasonCode: "transport_parity"
  ) {{
    groupId membershipId userId membershipRevision groupVersion memberCount
    effectiveStatus enforcementRevision effectiveUntil revokedAt replayed
  }}
}}
"#
            ),
        )
        .await,
    );
    assert_graphql_result(
        &graphql_suspend_after_revoke["suspendGroupMembership"],
        &native_suspend_after_revoke,
        graphql_group_id,
        graphql_target_membership_id,
        true,
    );

    let native_final = owner_snapshot(&db, tenant_id, native_group_id, target_id).await;
    let graphql_final = owner_snapshot(&db, tenant_id, graphql_group_id, target_id).await;
    assert_eq!(native_final, graphql_final);
    assert_eq!(native_final.0, native_revoke.group_version);
    assert_eq!(native_final.1, 2);
    assert_eq!(native_final.2, "member");
    assert_eq!(native_final.3, "active");
    assert_eq!(native_final.4, 3);
    assert_eq!(native_final.5, native_revoke.enforcement_revision);
    assert_eq!(native_final.6, "direct_local");
    assert_eq!(native_final.7, 1);

    drop(schema);
    drop(native);
    drop(db);
    drop(temp);
}
