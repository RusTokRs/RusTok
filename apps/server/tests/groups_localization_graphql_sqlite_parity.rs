#![cfg(feature = "mod-groups")]

use std::time::Duration;

use async_graphql::{EmptySubscription, Request, Response, Schema};
use rustok_api::{AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext};
use rustok_groups::graphql_application_cas::{GroupsMutationRoot, GroupsQueryRoot};
use rustok_groups::{
    DeleteGroupTranslationRequest, GroupLocalizationCommandPort, GroupLocalizationReadPort,
    GroupLocalizationService, ListGroupTranslationsRequest, UpsertGroupTranslationRequest,
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
        .expect("Groups localization GraphQL parity SQLite connection should open")
}

async fn install_groups_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_groups::migrations::migrations() {
        migration.up(&manager).await.expect(
            "production Groups migration should apply for localization GraphQL parity evidence",
        );
    }
}

fn sqlite_fixture_url(temp: &TempDir) -> String {
    let path = temp
        .path()
        .join("groups-localization-graphql-parity.sqlite");
    format!("sqlite://{}?mode=rwc", path.display())
}

async fn seed_group_fixture(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    native_group_id: Uuid,
    graphql_group_id: Uuid,
    owner_id: Uuid,
) {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO groups (id, tenant_id, owner_user_id, handle, member_count)
VALUES
    ('{native_group_id}', '{tenant_id}', '{owner_id}', 'localization-native-parity', 1),
    ('{graphql_group_id}', '{tenant_id}', '{owner_id}', 'localization-graphql-parity', 1);

INSERT INTO group_memberships
    (id, tenant_id, group_id, user_id, role, status, joined_at)
VALUES
    ('{}', '{tenant_id}', '{native_group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP),
    ('{}', '{tenant_id}', '{graphql_group_id}', '{owner_id}', 'owner', 'active', CURRENT_TIMESTAMP);
"#,
        Uuid::new_v4(),
        Uuid::new_v4(),
    ))
    .await
    .expect("Groups localization GraphQL parity fixture should seed");
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Groups localization GraphQL parity".to_string(),
        slug: "groups-localization-graphql-parity".to_string(),
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

fn native_read_context(tenant_id: Uuid, owner_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(owner_id.to_string()),
        "en",
        format!("groups-localization-native-read-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(5))
}

fn native_write_context(tenant_id: Uuid, owner_id: Uuid, operation: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(owner_id.to_string()),
        "en",
        format!("groups-localization-native-write-{}", Uuid::new_v4()),
    )
    .with_deadline(Duration::from_secs(5))
    .with_idempotency_key(format!("{operation}-{}", Uuid::new_v4()))
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
        "Groups localization GraphQL parity request should succeed: {:?}",
        response.errors
    );
    response
        .data
        .into_json()
        .expect("Groups localization GraphQL parity data should convert to JSON")
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
async fn localization_native_and_final_graphql_share_owner_semantics_sqlite() {
    let temp = tempfile::tempdir()
        .expect("temporary Groups localization GraphQL parity directory should create");
    let url = sqlite_fixture_url(&temp);
    let db = connect(&url).await;
    install_groups_schema(&db).await;

    let tenant_id = Uuid::new_v4();
    let native_group_id = Uuid::new_v4();
    let graphql_group_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    seed_group_fixture(&db, tenant_id, native_group_id, graphql_group_id, owner_id).await;

    let native = GroupLocalizationService::new(db.clone());
    let schema = graphql_schema(db.clone());

    let native_en = GroupLocalizationCommandPort::upsert_group_translation(
        &native,
        native_write_context(tenant_id, owner_id, "native-en"),
        UpsertGroupTranslationRequest {
            group_id: native_group_id,
            locale: "en".to_string(),
            title: "English title".to_string(),
            summary: Some("English summary".to_string()),
            body: Some("English body".to_string()),
        },
    )
    .await
    .expect("native English localization should succeed");

    let graphql_en = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            format!(
                r#"
mutation {{
  upsertGroupTranslation(
    idempotencyKey: "graphql-en",
    groupId: "{graphql_group_id}",
    input: {{
      locale: "en",
      title: "English title",
      summary: "English summary",
      body: "English body"
    }}
  ) {{
    translation {{ groupId locale title summary body }}
    groupVersion
    created
  }}
}}
"#
            ),
        )
        .await,
    );
    let graphql_en = &graphql_en["upsertGroupTranslation"];
    assert_eq!(graphql_en["created"].as_bool(), Some(native_en.created));
    assert_eq!(
        graphql_en["groupVersion"].as_u64(),
        Some(native_en.group_version)
    );
    assert_eq!(
        graphql_en["translation"]["locale"].as_str(),
        Some(native_en.translation.locale.as_str())
    );
    assert_eq!(
        graphql_en["translation"]["title"].as_str(),
        Some(native_en.translation.title.as_str())
    );
    assert_eq!(
        graphql_en["translation"]["summary"].as_str(),
        native_en.translation.summary.as_deref()
    );
    assert_eq!(
        graphql_en["translation"]["body"].as_str(),
        native_en.translation.body.as_deref()
    );
    assert_eq!(
        graphql_en["translation"]["groupId"]
            .as_str()
            .map(str::to_owned),
        Some(graphql_group_id.to_string())
    );

    let native_fr = GroupLocalizationCommandPort::upsert_group_translation(
        &native,
        native_write_context(tenant_id, owner_id, "native-fr"),
        UpsertGroupTranslationRequest {
            group_id: native_group_id,
            locale: "fr".to_string(),
            title: "Titre français".to_string(),
            summary: None,
            body: Some("Corps français".to_string()),
        },
    )
    .await
    .expect("native French localization should succeed");

    let graphql_fr = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            format!(
                r#"
mutation {{
  upsertGroupTranslation(
    idempotencyKey: "graphql-fr",
    groupId: "{graphql_group_id}",
    input: {{ locale: "fr", title: "Titre français", body: "Corps français" }}
  ) {{
    translation {{ locale title summary body }}
    groupVersion
    created
  }}
}}
"#
            ),
        )
        .await,
    );
    let graphql_fr = &graphql_fr["upsertGroupTranslation"];
    assert_eq!(graphql_fr["created"].as_bool(), Some(native_fr.created));
    assert_eq!(
        graphql_fr["groupVersion"].as_u64(),
        Some(native_fr.group_version)
    );
    assert_eq!(
        graphql_fr["translation"]["locale"].as_str(),
        Some(native_fr.translation.locale.as_str())
    );
    assert_eq!(
        graphql_fr["translation"]["title"].as_str(),
        Some(native_fr.translation.title.as_str())
    );
    assert!(graphql_fr["translation"]["summary"].is_null());
    assert_eq!(
        graphql_fr["translation"]["body"].as_str(),
        native_fr.translation.body.as_deref()
    );

    let native_list = GroupLocalizationReadPort::list_group_translations(
        &native,
        native_read_context(tenant_id, owner_id),
        ListGroupTranslationsRequest {
            group_id: native_group_id,
        },
    )
    .await
    .expect("native localization read should succeed");
    let graphql_list = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            format!(
                r#"
query {{
  groupTranslations(groupId: "{graphql_group_id}") {{ locale title summary body }}
}}
"#
            ),
        )
        .await,
    );
    let graphql_list = graphql_list["groupTranslations"]
        .as_array()
        .expect("GraphQL localization list should be an array");
    assert_eq!(graphql_list.len(), native_list.len());
    for (graphql_item, native_item) in graphql_list.iter().zip(native_list.iter()) {
        assert_eq!(
            graphql_item["locale"].as_str(),
            Some(native_item.locale.as_str())
        );
        assert_eq!(
            graphql_item["title"].as_str(),
            Some(native_item.title.as_str())
        );
        assert_eq!(
            graphql_item["summary"].as_str(),
            native_item.summary.as_deref()
        );
        assert_eq!(graphql_item["body"].as_str(), native_item.body.as_deref());
    }

    let native_delete = GroupLocalizationCommandPort::delete_group_translation(
        &native,
        native_write_context(tenant_id, owner_id, "native-delete-fr"),
        DeleteGroupTranslationRequest {
            group_id: native_group_id,
            locale: "fr".to_string(),
        },
    )
    .await
    .expect("native French localization delete should succeed");
    let graphql_delete = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            format!(
                r#"
mutation {{
  deleteGroupTranslation(
    idempotencyKey: "graphql-delete-fr",
    groupId: "{graphql_group_id}",
    locale: "fr"
  ) {{ groupId locale groupVersion }}
}}
"#
            ),
        )
        .await,
    );
    let graphql_delete = &graphql_delete["deleteGroupTranslation"];
    assert_eq!(
        graphql_delete["groupId"].as_str().map(str::to_owned),
        Some(graphql_group_id.to_string())
    );
    assert_eq!(
        graphql_delete["locale"].as_str(),
        Some(native_delete.locale.as_str())
    );
    assert_eq!(
        graphql_delete["groupVersion"].as_u64(),
        Some(native_delete.group_version)
    );

    let native_last_error = GroupLocalizationCommandPort::delete_group_translation(
        &native,
        native_write_context(tenant_id, owner_id, "native-delete-last"),
        DeleteGroupTranslationRequest {
            group_id: native_group_id,
            locale: "en".to_string(),
        },
    )
    .await
    .expect_err("native last translation delete must fail closed");
    assert_eq!(native_last_error.code, "groups.conflict");
    assert!(!native_last_error.retryable);

    let graphql_last = execute_graphql(
        &schema,
        tenant_id,
        owner_id,
        format!(
            r#"
mutation {{
  deleteGroupTranslation(
    idempotencyKey: "graphql-delete-last",
    groupId: "{graphql_group_id}",
    locale: "en"
  ) {{ groupVersion }}
}}
"#
        ),
    )
    .await;
    assert_eq!(graphql_last.errors.len(), 1);
    let graphql_last_error = &graphql_last.errors[0];
    assert_eq!(graphql_last_error.message, native_last_error.message);
    assert_eq!(
        extension_json(graphql_last_error, "code")
            .and_then(|value| value.as_str().map(str::to_owned)),
        Some("BAD_USER_INPUT".to_string())
    );

    let native_final = GroupLocalizationReadPort::list_group_translations(
        &native,
        native_read_context(tenant_id, owner_id),
        ListGroupTranslationsRequest {
            group_id: native_group_id,
        },
    )
    .await
    .expect("native final localization read should succeed");
    let graphql_final = response_json(
        execute_graphql(
            &schema,
            tenant_id,
            owner_id,
            format!(
                r#"
query {{
  groupTranslations(groupId: "{graphql_group_id}") {{ locale title }}
}}
"#
            ),
        )
        .await,
    );
    assert_eq!(native_final.len(), 1);
    assert_eq!(native_final[0].locale, "en");
    let graphql_final = graphql_final["groupTranslations"]
        .as_array()
        .expect("final GraphQL localization list should be an array");
    assert_eq!(graphql_final.len(), 1);
    assert_eq!(graphql_final[0]["locale"].as_str(), Some("en"));

    drop(schema);
    drop(native);
    drop(db);
    drop(temp);
}
