#![cfg(feature = "mod-groups")]

use std::time::Duration;

use async_graphql::{EmptySubscription, Request, Response, Schema};
use rustok_api::{AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext};
use rustok_groups::graphql_application_cas::{GroupsMutationRoot, GroupsQueryRoot};
use rustok_groups::{
    GroupMembershipEffectiveStatus, GroupMembershipEnforcementCommandPort,
    GroupMembershipEnforcementCommandService, RevokeGroupMembershipSuspensionRequest,
    SuspendGroupMembershipRequest,
};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
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
        .expect("Groups GraphQL parity SQLite connection should open")
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("production Groups migration should apply for GraphQL parity evidence");
    }
}

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp.path().join("groups-enforcement-graphql-parity.sqlite");
    format!("sqlite://{}?mode=rwc", path.display())
}

async fn seed_group_fixture(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    group_id: Uuid,
    owner_id: Uuid,
    native_target_id: Uuid,
    graphql_target_id: Uuid,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES ('{group_id}', '{tenant_id}', '{owner_id}', 'enforcement-graphql-parity', 3);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{tenant_id}', '{group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{native_target_id}', 'member', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{group_id}', '{graphql_target_id}', 'member', 'active', CURRENT_TIMESTAMP);
"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("Groups GraphQL parity fixture should seed");
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Groups GraphQL parity".to_string(),
        slug: "groups-graphql-parity".to_string(),
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

fn native_context(tenant_id: Uuid, owner_id: Uuid, idempotency_key: &str) -> PortContext {
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
        "GraphQL parity request should succeed: {:?}",
        response.errors
    );
    response
        .data
        .into_json()
        .expect("GraphQL parity response data should convert to JSON")
}

fn extension_json(error: &async_graphql::ServerError, key: &str) -> Option<serde_json::Value> {
    error
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get(key))
        .cloned()
        .and_then(|value| value.into_json().ok())
}

#[tokio::test]
async fn direct_enforcement_native_and_graphql_share_owner_semantics_sqlite() {
    let temp = tempfile::tempdir().expect("temporary Groups GraphQL parity directory should create");
    let url = sqlite_fixture_url(&temp);
    let db = connect(&url).await;
    install_groups_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let native_target_id = Uuid::new_v4();
    let graphql_target_id = Uuid::new_v4();
    seed_group_fixture(
        &db,
        tenant_id,
        group_id,
        owner_id,
        native_target_id,
        graphql_target_id,
    )
    .await;

    let native_service = GroupMembershipEnforcementCommandService::new(db.clone());
    let native_suspend = GroupMembershipEnforcementCommandPort::suspend_membership(
        &native_service,
        native_context(tenant_id, owner_id, "native-suspend"),
        SuspendGroupMembershipRequest {
            group_id,
            target_user_id: native_target_id,
            expected_membership_revision: 1,
            reason_code: "parity_review".to_string(),
            effective_until: None,
        },
    )
    .await
    .expect("native owner suspend should succeed");
    assert_eq!(
        native_suspend.effective_status,
        GroupMembershipEffectiveStatus::Suspended
    );
    assert_eq!(native_suspend.membership_revision, 2);
    assert_eq!(native_suspend.enforcement_revision, 1);
    assert_eq!(native_suspend.member_count, 3);
    assert!(!native_suspend.replayed);

    let schema = graphql_schema(db.clone());
    let graphql_suspend_document = format!(
        r#"
mutation {{
  suspendGroupMembership(
    idempotencyKey: "graphql-suspend",
    groupId: "{group_id}",
    targetUserId: "{graphql_target_id}",
    expectedMembershipRevision: 1,
    reasonCode: "parity_review"
  ) {{
    groupId
    membershipId
    userId
    membershipRevision
    groupVersion
    memberCount
    effectiveStatus
    enforcementRevision
    effectiveUntil
    revokedAt
    replayed
  }}
}}
"#
    );
    let graphql_suspend = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            graphql_suspend_document.clone(),
        )
        .await,
    );
    let graphql_suspend = &graphql_suspend["suspendGroupMembership"];
    assert_eq!(graphql_suspend["groupId"], group_id.to_string());
    assert_eq!(graphql_suspend["userId"], graphql_target_id.to_string());
    assert_eq!(
        graphql_suspend["membershipRevision"].as_i64(),
        Some(native_suspend.membership_revision)
    );
    assert_eq!(
        graphql_suspend["memberCount"].as_i64(),
        Some(native_suspend.member_count)
    );
    assert_eq!(
        graphql_suspend["effectiveStatus"].as_str(),
        Some(native_suspend.effective_status.as_str())
    );
    assert_eq!(
        graphql_suspend["enforcementRevision"].as_i64(),
        Some(native_suspend.enforcement_revision)
    );
    assert_eq!(graphql_suspend["replayed"].as_bool(), Some(false));
    assert!(graphql_suspend["effectiveUntil"].is_null());
    assert!(graphql_suspend["revokedAt"].is_null());
    assert!(
        graphql_suspend["groupVersion"].as_i64().is_some_and(|version| version > native_suspend.group_version),
        "sequential GraphQL mutation should return a later owner group version"
    );

    let replay = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            graphql_suspend_document,
        )
        .await,
    );
    let replay = &replay["suspendGroupMembership"];
    assert_eq!(replay["membershipId"], graphql_suspend["membershipId"]);
    assert_eq!(
        replay["membershipRevision"],
        graphql_suspend["membershipRevision"]
    );
    assert_eq!(replay["groupVersion"], graphql_suspend["groupVersion"]);
    assert_eq!(replay["memberCount"], graphql_suspend["memberCount"]);
    assert_eq!(replay["effectiveStatus"], graphql_suspend["effectiveStatus"]);
    assert_eq!(replay["enforcementRevision"], graphql_suspend["enforcementRevision"]);
    assert_eq!(replay["replayed"].as_bool(), Some(true));

    let stale = execute_graphql(
        &schema,
        tenant_id,
        owner_id,
        format!(
            r#"
mutation {{
  suspendGroupMembership(
    idempotencyKey: "graphql-stale",
    groupId: "{group_id}",
    targetUserId: "{graphql_target_id}",
    expectedMembershipRevision: 1,
    reasonCode: "stale_review"
  ) {{ membershipRevision }}
}}
"#
        ),
    )
    .await;
    assert_eq!(stale.errors.len(), 1);
    let stale_error = &stale.errors[0];
    assert_eq!(
        extension_json(stale_error, "code").and_then(|value| value.as_str().map(str::to_owned)),
        Some("BAD_USER_INPUT".to_string())
    );
    assert_eq!(
        extension_json(stale_error, "domainCode")
            .and_then(|value| value.as_str().map(str::to_owned)),
        Some("groups.membership_enforcement_revision_conflict".to_string())
    );
    assert_eq!(
        extension_json(stale_error, "retryable").and_then(|value| value.as_bool()),
        Some(false)
    );

    let native_revoke = GroupMembershipEnforcementCommandPort::revoke_membership_suspension(
        &native_service,
        native_context(tenant_id, owner_id, "native-revoke"),
        RevokeGroupMembershipSuspensionRequest {
            group_id,
            target_user_id: native_target_id,
            expected_membership_revision: native_suspend.membership_revision,
            reason_code: "parity_complete".to_string(),
        },
    )
    .await
    .expect("native owner revoke should succeed");
    assert_eq!(native_revoke.effective_status, GroupMembershipEffectiveStatus::Active);
    assert_eq!(native_revoke.membership_revision, 3);
    assert_eq!(native_revoke.enforcement_revision, 2);
    assert_eq!(native_revoke.member_count, 3);
    assert!(native_revoke.revoked_at.is_some());

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
    groupId: "{group_id}",
    targetUserId: "{graphql_target_id}",
    expectedMembershipRevision: 2,
    reasonCode: "parity_complete"
  ) {{
    membershipRevision
    groupVersion
    memberCount
    effectiveStatus
    enforcementRevision
    effectiveUntil
    revokedAt
    replayed
  }}
}}
"#
            ),
        )
        .await,
    );
    let graphql_revoke = &graphql_revoke["revokeGroupMembershipSuspension"];
    assert_eq!(
        graphql_revoke["membershipRevision"].as_i64(),
        Some(native_revoke.membership_revision)
    );
    assert_eq!(
        graphql_revoke["memberCount"].as_i64(),
        Some(native_revoke.member_count)
    );
    assert_eq!(
        graphql_revoke["effectiveStatus"].as_str(),
        Some(native_revoke.effective_status.as_str())
    );
    assert_eq!(
        graphql_revoke["enforcementRevision"].as_i64(),
        Some(native_revoke.enforcement_revision)
    );
    assert!(graphql_revoke["effectiveUntil"].is_null());
    assert!(graphql_revoke["revokedAt"].is_string());
    assert_eq!(graphql_revoke["replayed"].as_bool(), Some(false));
    assert!(
        graphql_revoke["groupVersion"].as_i64().is_some_and(|version| version > native_revoke.group_version),
        "sequential GraphQL revoke should return a later owner group version"
    );

    drop(native_service);
    drop(schema);
    drop(db);
    drop(temp);
}
