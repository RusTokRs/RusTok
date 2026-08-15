#![cfg(feature = "mod-groups")]

use std::time::Duration;

use async_graphql::{EmptySubscription, Request, Response, Schema};
use rustok_api::{
    AuthContext, HostRuntimeContext, PortActor, PortContext, PortErrorKind, TenantContext,
};
use rustok_groups::graphql_application_cas::{GroupsMutationRoot, GroupsQueryRoot};
use rustok_groups::{
    ChangeGroupRoleRequest, GroupGovernanceCommandPort, GroupGovernanceService, GroupRole,
    TransferGroupOwnershipRequest,
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
        .expect("Groups governance GraphQL parity SQLite connection should open")
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration.up(&manager).await.expect(
            "production Groups migration should apply for governance GraphQL parity evidence",
        );
    }
}

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp.path().join("groups-governance-graphql-parity.sqlite");
    format!("sqlite://{}?mode=rwc", path.display())
}

async fn seed_group_fixture(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    native_group_id: Uuid,
    graphql_group_id: Uuid,
    owner_id: Uuid,
    admin_id: Uuid,
    target_id: Uuid,
    replacement_id: Uuid,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES
    ('{native_group_id}', '{tenant_id}', '{owner_id}', 'governance-native-parity', 4),
    ('{graphql_group_id}', '{tenant_id}', '{owner_id}', 'governance-graphql-parity', 4);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{tenant_id}', '{native_group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{native_group_id}', '{admin_id}', 'admin', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{native_group_id}', '{target_id}', 'member', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{native_group_id}', '{replacement_id}', 'member', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{graphql_group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{graphql_group_id}', '{admin_id}', 'admin', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{graphql_group_id}', '{target_id}', 'member', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{graphql_group_id}', '{replacement_id}', 'member', 'active', CURRENT_TIMESTAMP);
"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("Groups governance GraphQL parity fixture should seed");
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Groups governance GraphQL parity".to_string(),
        slug: "groups-governance-graphql-parity".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}

fn auth_context(tenant_id: Uuid, user_id: Uuid) -> AuthContext {
    AuthContext {
        user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: Vec::new(),
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    }
}

fn native_write_context(tenant_id: Uuid, actor_id: Uuid, idempotency_key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("groups-governance-native-parity-{}", Uuid::new_v4()),
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
    actor_id: Uuid,
    document: String,
) -> Response {
    schema
        .execute(
            Request::new(document)
                .data(tenant_context(tenant_id))
                .data(auth_context(tenant_id, actor_id)),
        )
        .await
}

fn response_json(response: Response) -> serde_json::Value {
    assert!(
        response.errors.is_empty(),
        "Groups governance GraphQL parity request should succeed: {:?}",
        response.errors
    );
    response
        .data
        .into_json()
        .expect("Groups governance GraphQL parity data should convert to JSON")
}

fn extension_json(error: &async_graphql::ServerError, key: &str) -> Option<serde_json::Value> {
    error
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get(key))
        .cloned()
        .and_then(|value| value.into_json().ok())
}

async fn group_snapshot(db: &DatabaseConnection, tenant_id: Uuid, group_id: Uuid) -> (String, i64) {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT owner_user_id, version FROM groups WHERE tenant_id = '{tenant_id}' AND id = '{group_id}'"
            ),
        ))
        .await
        .expect("group snapshot query should succeed")
        .expect("group should exist");
    (
        row.try_get("", "owner_user_id")
            .expect("group owner should decode"),
        row.try_get("", "version")
            .expect("group version should decode"),
    )
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

fn assert_graphql_governance_result(
    graphql: &serde_json::Value,
    expected_group_id: Uuid,
    expected_actor_id: Uuid,
    expected_target_id: Uuid,
    expected_previous_role: &str,
    expected_current_role: &str,
    expected_group_version: u64,
    expected_replayed: bool,
) {
    assert_eq!(
        graphql["groupId"].as_str().map(str::to_owned),
        Some(expected_group_id.to_string())
    );
    assert_eq!(
        graphql["actorUserId"].as_str().map(str::to_owned),
        Some(expected_actor_id.to_string())
    );
    assert_eq!(
        graphql["targetUserId"].as_str().map(str::to_owned),
        Some(expected_target_id.to_string())
    );
    assert_eq!(
        graphql["previousRole"].as_str(),
        Some(expected_previous_role)
    );
    assert_eq!(graphql["currentRole"].as_str(), Some(expected_current_role));
    assert_eq!(
        graphql["groupVersion"].as_u64(),
        Some(expected_group_version)
    );
    assert_eq!(graphql["replayed"].as_bool(), Some(expected_replayed));
}

#[tokio::test]
async fn governance_native_and_final_graphql_share_owner_semantics_sqlite() {
    let temp = tempfile::tempdir()
        .expect("temporary Groups governance GraphQL parity directory should create");
    let url = sqlite_fixture_url(&temp);
    let db = connect(&url).await;
    install_groups_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let native_group_id = Uuid::new_v4();
    let graphql_group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    let replacement_id = Uuid::new_v4();
    seed_group_fixture(
        &db,
        tenant_id,
        native_group_id,
        graphql_group_id,
        owner_id,
        admin_id,
        target_id,
        replacement_id,
    )
    .await;

    let native = GroupGovernanceService::new(db.clone());
    let schema = graphql_schema(db.clone());

    let native_role = GroupGovernanceCommandPort::change_group_role(
        &native,
        native_write_context(tenant_id, admin_id, "native-change-role"),
        ChangeGroupRoleRequest {
            group_id: native_group_id,
            target_user_id: target_id,
            role: GroupRole::Moderator,
        },
    )
    .await
    .expect("native administrator role change should succeed");
    assert_eq!(native_role.previous_role, GroupRole::Member);
    assert_eq!(native_role.current_role, GroupRole::Moderator);
    assert!(!native_role.replayed);

    let graphql_role = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            admin_id,
            format!(
                r#"
mutation {{
  changeGroupRole(
    idempotencyKey: "graphql-change-role",
    groupId: "{graphql_group_id}",
    targetUserId: "{target_id}",
    role: MODERATOR
  ) {{
    groupId actorUserId targetUserId previousRole currentRole groupVersion replayed
  }}
}}
"#
            ),
        )
        .await,
    );
    assert_graphql_governance_result(
        &graphql_role["changeGroupRole"],
        graphql_group_id,
        admin_id,
        target_id,
        "MEMBER",
        "MODERATOR",
        native_role.group_version,
        false,
    );

    let native_role_replay = GroupGovernanceCommandPort::change_group_role(
        &native,
        native_write_context(tenant_id, admin_id, "native-change-role"),
        ChangeGroupRoleRequest {
            group_id: native_group_id,
            target_user_id: target_id,
            role: GroupRole::Moderator,
        },
    )
    .await
    .expect("native role change replay should succeed");
    assert!(native_role_replay.replayed);
    assert_eq!(native_role_replay.group_version, native_role.group_version);

    let graphql_role_replay = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            admin_id,
            format!(
                r#"
mutation {{
  changeGroupRole(
    idempotencyKey: "graphql-change-role",
    groupId: "{graphql_group_id}",
    targetUserId: "{target_id}",
    role: MODERATOR
  ) {{
    groupId actorUserId targetUserId previousRole currentRole groupVersion replayed
  }}
}}
"#
            ),
        )
        .await,
    );
    assert_graphql_governance_result(
        &graphql_role_replay["changeGroupRole"],
        graphql_group_id,
        admin_id,
        target_id,
        "MEMBER",
        "MODERATOR",
        native_role.group_version,
        true,
    );

    let native_forbidden = GroupGovernanceCommandPort::transfer_group_ownership(
        &native,
        native_write_context(tenant_id, admin_id, "native-forbidden-transfer"),
        TransferGroupOwnershipRequest {
            group_id: native_group_id,
            new_owner_user_id: replacement_id,
        },
    )
    .await
    .expect_err("non-owner administrator must not transfer ownership");
    assert_eq!(native_forbidden.kind, PortErrorKind::Forbidden);
    assert_eq!(native_forbidden.code, "groups.forbidden");
    assert!(!native_forbidden.retryable);

    let graphql_forbidden = execute_graphql(
        &schema,
        tenant_id,
        admin_id,
        format!(
            r#"
mutation {{
  transferGroupOwnership(
    idempotencyKey: "graphql-forbidden-transfer",
    groupId: "{graphql_group_id}",
    newOwnerUserId: "{replacement_id}"
  ) {{ groupVersion }}
}}
"#
        ),
    )
    .await;
    assert_eq!(graphql_forbidden.errors.len(), 1);
    let graphql_forbidden_error = &graphql_forbidden.errors[0];
    assert_eq!(graphql_forbidden_error.message, native_forbidden.message);
    assert_eq!(
        extension_json(graphql_forbidden_error, "code")
            .and_then(|value| value.as_str().map(str::to_owned)),
        Some("PERMISSION_DENIED".to_string())
    );

    let (native_owner_before_transfer, native_version_before_transfer) =
        group_snapshot(&db, tenant_id, native_group_id).await;
    let (graphql_owner_before_transfer, graphql_version_before_transfer) =
        group_snapshot(&db, tenant_id, graphql_group_id).await;
    assert_eq!(native_owner_before_transfer, owner_id.to_string());
    assert_eq!(graphql_owner_before_transfer, owner_id.to_string());
    assert_eq!(
        native_version_before_transfer as u64,
        native_role.group_version
    );
    assert_eq!(
        graphql_version_before_transfer as u64,
        native_role.group_version
    );

    let native_transfer = GroupGovernanceCommandPort::transfer_group_ownership(
        &native,
        native_write_context(tenant_id, owner_id, "native-transfer-owner"),
        TransferGroupOwnershipRequest {
            group_id: native_group_id,
            new_owner_user_id: replacement_id,
        },
    )
    .await
    .expect("native owner transfer should succeed");
    assert_eq!(native_transfer.previous_role, GroupRole::Member);
    assert_eq!(native_transfer.current_role, GroupRole::Owner);
    assert_eq!(native_transfer.group_version, native_role.group_version + 1);
    assert!(!native_transfer.replayed);

    let graphql_transfer = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            format!(
                r#"
mutation {{
  transferGroupOwnership(
    idempotencyKey: "graphql-transfer-owner",
    groupId: "{graphql_group_id}",
    newOwnerUserId: "{replacement_id}"
  ) {{
    groupId actorUserId targetUserId previousRole currentRole groupVersion replayed
  }}
}}
"#
            ),
        )
        .await,
    );
    assert_graphql_governance_result(
        &graphql_transfer["transferGroupOwnership"],
        graphql_group_id,
        owner_id,
        replacement_id,
        "MEMBER",
        "OWNER",
        native_transfer.group_version,
        false,
    );

    let native_transfer_replay = GroupGovernanceCommandPort::transfer_group_ownership(
        &native,
        native_write_context(tenant_id, owner_id, "native-transfer-owner"),
        TransferGroupOwnershipRequest {
            group_id: native_group_id,
            new_owner_user_id: replacement_id,
        },
    )
    .await
    .expect("native ownership transfer replay should succeed");
    assert!(native_transfer_replay.replayed);
    assert_eq!(
        native_transfer_replay.group_version,
        native_transfer.group_version
    );

    let graphql_transfer_replay = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            format!(
                r#"
mutation {{
  transferGroupOwnership(
    idempotencyKey: "graphql-transfer-owner",
    groupId: "{graphql_group_id}",
    newOwnerUserId: "{replacement_id}"
  ) {{
    groupId actorUserId targetUserId previousRole currentRole groupVersion replayed
  }}
}}
"#
            ),
        )
        .await,
    );
    assert_graphql_governance_result(
        &graphql_transfer_replay["transferGroupOwnership"],
        graphql_group_id,
        owner_id,
        replacement_id,
        "MEMBER",
        "OWNER",
        native_transfer.group_version,
        true,
    );

    let (native_owner_after, native_version_after) =
        group_snapshot(&db, tenant_id, native_group_id).await;
    let (graphql_owner_after, graphql_version_after) =
        group_snapshot(&db, tenant_id, graphql_group_id).await;
    assert_eq!(native_owner_after, replacement_id.to_string());
    assert_eq!(graphql_owner_after, replacement_id.to_string());
    assert_eq!(native_version_after as u64, native_transfer.group_version);
    assert_eq!(graphql_version_after as u64, native_transfer.group_version);

    for group_id in [native_group_id, graphql_group_id] {
        assert_eq!(
            membership_role(&db, tenant_id, group_id, owner_id).await,
            "admin"
        );
        assert_eq!(
            membership_role(&db, tenant_id, group_id, replacement_id).await,
            "owner"
        );
        assert_eq!(
            membership_role(&db, tenant_id, group_id, target_id).await,
            "moderator"
        );
        assert_eq!(
            membership_role(&db, tenant_id, group_id, admin_id).await,
            "admin"
        );
    }

    drop(schema);
    drop(native);
    drop(db);
    drop(temp);
}
