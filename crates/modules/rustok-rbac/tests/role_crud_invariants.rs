use async_graphql::{EmptySubscription, Schema};
use rustok_api::{AuthContext, AuthPrincipalContext, AuthPrincipalKind, Permission, TenantContext};
use rustok_rbac::graphql::{RbacMutation, RbacQuery};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use uuid::Uuid;

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect sqlite memory");

    db.execute_unprepared(
        r#"
        CREATE TABLE tenants (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            slug TEXT UNIQUE NOT NULL
        );
        CREATE TABLE users (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL
        );
        CREATE TABLE roles (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            name TEXT NOT NULL,
            slug TEXT NOT NULL,
            description TEXT,
            is_system BOOLEAN NOT NULL DEFAULT 0,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(tenant_id, slug)
        );
        CREATE TABLE permissions (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            resource TEXT NOT NULL,
            action TEXT NOT NULL,
            description TEXT,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(tenant_id, resource, action)
        );
        CREATE TABLE role_permissions (
            id TEXT PRIMARY KEY NOT NULL,
            role_id TEXT NOT NULL,
            permission_id TEXT NOT NULL,
            UNIQUE(role_id, permission_id)
        );
        CREATE TABLE user_roles (
            id TEXT PRIMARY KEY NOT NULL,
            user_id TEXT NOT NULL,
            role_id TEXT NOT NULL,
            UNIQUE(user_id, role_id)
        );
        CREATE TABLE rbac_invalidation_state (
            scope TEXT PRIMARY KEY NOT NULL,
            generation BIGINT NOT NULL,
            updated_at TEXT NOT NULL
        );
        INSERT INTO rbac_invalidation_state (scope, generation, updated_at)
        VALUES ('permissions', 0, CURRENT_TIMESTAMP);
        CREATE TABLE rbac_artifact_role_permissions (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            role_id TEXT NOT NULL,
            installation_id TEXT NOT NULL,
            permission_key TEXT NOT NULL,
            granted_by_actor_id TEXT NOT NULL,
            granted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (tenant_id, role_id, installation_id, permission_key)
        );
        CREATE TABLE rbac_artifact_role_permission_operations (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            role_id TEXT NOT NULL,
            installation_id TEXT NOT NULL,
            permission_key TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            granted BOOLEAN NOT NULL,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (tenant_id, idempotency_key)
        );
        "#,
    )
    .await
    .expect("create test tables");

    db
}

fn test_schema(
    db: DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    permissions: Vec<Permission>,
) -> Schema<RbacQuery, RbacMutation, EmptySubscription> {
    let auth = AuthContext {
        user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions,
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    };
    let principal = AuthPrincipalContext::new(AuthPrincipalKind::DirectUser);
    let tenant = TenantContext {
        id: tenant_id,
        name: "Test Tenant".to_string(),
        slug: "test-tenant".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };

    Schema::build(RbacQuery, RbacMutation, EmptySubscription)
        .data(db)
        .data(auth)
        .data(principal)
        .data(tenant)
        .finish()
}

#[tokio::test]
async fn test_platform_permissions_query_returns_catalog() {
    let db = setup_test_db().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let schema = test_schema(
        db,
        tenant_id,
        user_id,
        vec![Permission::SETTINGS_READ, Permission::SETTINGS_MANAGE],
    );

    let query = r#"
        query {
            platformPermissions {
                id
                resource
                action
            }
        }
    "#;

    let res = schema.execute(query).await;
    assert!(res.errors.is_empty(), "Errors: {:?}", res.errors);

    let data = res.data.into_json().expect("json data");
    let permissions = data["platformPermissions"]
        .as_array()
        .expect("platformPermissions array");

    assert!(
        permissions.len() > 50,
        "Platform permissions should contain dozens of resources and actions"
    );

    // Verify key platform resources are represented
    let ids: Vec<&str> = permissions
        .iter()
        .filter_map(|p| p["id"].as_str())
        .collect();
    assert!(ids.contains(&"users:create"));
    assert!(ids.contains(&"users:manage"));
    assert!(ids.contains(&"settings:manage"));
    assert!(ids.contains(&"analytics:export"));
}

#[tokio::test]
async fn test_custom_role_lifecycle_and_system_role_invariants() {
    let db = setup_test_db().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    // 1. Create a system role and a custom role
    let schema = test_schema(
        db.clone(),
        tenant_id,
        user_id,
        vec![
            Permission::SETTINGS_READ,
            Permission::SETTINGS_MANAGE,
            // The actor may only grant permissions it effectively holds
            // (authority ceiling), so the fixture carries the grants the
            // lifecycle below assigns to the custom role.
            Permission::POSTS_CREATE,
            Permission::POSTS_READ,
            Permission::POSTS_UPDATE,
            Permission::POSTS_DELETE,
            Permission::PAGES_READ,
        ],
    );

    // Seed system role super_admin
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        sea_orm::DbBackend::Sqlite,
        "INSERT INTO roles (id, tenant_id, name, slug, description, is_system) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            "Super Admin".into(),
            "super_admin".into(),
            "Built-in".into(),
            true.into(),
        ],
    ))
    .await
    .expect("insert system role");

    // Try to delete the super administrator role -> MUST FAIL
    let del_system_mutation = r#"
        mutation {
            deleteRole(slug: "super_admin") {
                success
                slug
            }
        }
    "#;
    let res = schema.execute(del_system_mutation).await;
    assert!(!res.errors.is_empty(), "Deleting super_admin must fail");
    assert!(
        res.errors[0]
            .message
            .contains("Cannot delete the super administrator role"),
        "Actual error: {}",
        res.errors[0].message
    );

    // A seeded default role other than super_admin is an ordinary tenant
    // record: it can be deleted while unassigned.
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        sea_orm::DbBackend::Sqlite,
        "INSERT INTO roles (id, tenant_id, name, slug, description, is_system) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            "Manager".into(),
            "manager".into(),
            "Built-in".into(),
            true.into(),
        ],
    ))
    .await
    .expect("insert default manager role");
    let del_default_mutation = r#"
        mutation {
            deleteRole(slug: "manager") {
                success
                slug
            }
        }
    "#;
    let res = schema.execute(del_default_mutation).await;
    assert!(
        res.errors.is_empty(),
        "Deleting an unassigned default role must succeed: {:?}",
        res.errors
    );

    // Try to restrict super_admin permissions -> MUST FAIL
    let update_superadmin_mutation = r#"
        mutation {
            updateRole(input: {
                slug: "super_admin",
                permissions: ["users:read"]
            }) {
                success
            }
        }
    "#;
    let res = schema.execute(update_superadmin_mutation).await;
    assert!(
        !res.errors.is_empty(),
        "Modifying super_admin permissions must fail"
    );
    assert!(
        res.errors[0]
            .message
            .contains("Super administrator permissions are immutable"),
        "Actual error: {}",
        res.errors[0].message
    );

    // 2. Create custom role -> MUST SUCCEED
    let create_custom_mutation = r#"
        mutation {
            createRole(input: {
                name: "Content Editor",
                slug: "content_editor",
                description: "Can manage posts and pages",
                permissions: ["posts:create", "posts:read", "pages:read"]
            }) {
                success
                role {
                    id
                    slug
                    displayName
                    description
                    isSystem
                    permissions
                }
            }
        }
    "#;
    let res = schema.execute(create_custom_mutation).await;
    assert!(res.errors.is_empty(), "Errors: {:?}", res.errors);
    let data = res.data.into_json().expect("json data");
    let role = &data["createRole"]["role"];
    assert_eq!(role["slug"], "content_editor");
    assert_eq!(role["displayName"], "Content Editor");
    assert_eq!(role["isSystem"], false);
    let perms = role["permissions"].as_array().expect("permissions array");
    assert_eq!(perms.len(), 3);

    // 3. Update custom role -> MUST SUCCEED
    let update_custom_mutation = r#"
        mutation {
            updateRole(input: {
                slug: "content_editor",
                name: "Senior Content Editor",
                permissions: ["posts:create", "posts:read", "posts:update", "posts:delete"]
            }) {
                success
                role {
                    displayName
                    permissions
                }
            }
        }
    "#;
    let res = schema.execute(update_custom_mutation).await;
    assert!(res.errors.is_empty(), "Errors: {:?}", res.errors);
    let data = res.data.into_json().expect("json data");
    assert_eq!(
        data["updateRole"]["role"]["displayName"],
        "Senior Content Editor"
    );
    assert_eq!(
        data["updateRole"]["role"]["permissions"]
            .as_array()
            .unwrap()
            .len(),
        4
    );

    // 4. Assign custom role to a user and try to delete -> MUST FAIL
    let target_user = Uuid::new_v4();
    let custom_role_id: Uuid = role["id"]
        .as_str()
        .expect("role id")
        .parse()
        .expect("parse uuid");
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        sea_orm::DbBackend::Sqlite,
        "INSERT INTO user_roles (id, user_id, role_id) VALUES (?1, ?2, ?3)",
        vec![
            Uuid::new_v4().into(),
            target_user.into(),
            custom_role_id.into(),
        ],
    ))
    .await
    .expect("assign role");

    let del_assigned_mutation = r#"
        mutation {
            deleteRole(slug: "content_editor") {
                success
                slug
            }
        }
    "#;
    let res = schema.execute(del_assigned_mutation).await;
    assert!(!res.errors.is_empty(), "Deleting assigned role must fail");
    assert!(
        res.errors[0]
            .message
            .contains("currently assigned to 1 user(s)"),
        "Actual error: {}",
        res.errors[0].message
    );

    // 5. Unassign user and delete custom role -> MUST SUCCEED
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
        sea_orm::DbBackend::Sqlite,
        "DELETE FROM user_roles WHERE role_id = ?1",
        vec![custom_role_id.into()],
    ))
    .await
    .expect("unassign role");

    let res = schema.execute(del_assigned_mutation).await;
    assert!(
        res.errors.is_empty(),
        "Deleting unassigned custom role must succeed: {:?}",
        res.errors
    );
    let data = res.data.into_json().expect("json data");
    assert_eq!(data["deleteRole"]["success"], true);
    assert_eq!(data["deleteRole"]["slug"], "content_editor");
}

#[tokio::test]
async fn test_role_mutation_conflict_and_not_found_semantics() {
    let db = setup_test_db().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let schema = test_schema(
        db.clone(),
        tenant_id,
        user_id,
        vec![Permission::SETTINGS_MANAGE, Permission::POSTS_READ],
    );

    let create_mutation = r#"
        mutation {
            createRole(input: {
                name: "Editor",
                slug: "editor",
                permissions: ["posts:read"]
            }) { success }
        }
    "#;
    let res = schema.execute(create_mutation).await;
    assert!(res.errors.is_empty(), "Errors: {:?}", res.errors);

    // Re-creating the same slug is a clean client-facing conflict, never a
    // raw database constraint error.
    let res = schema.execute(create_mutation).await;
    assert!(!res.errors.is_empty(), "Duplicate slug must fail");
    let message = &res.errors[0].message;
    assert!(
        message.contains("already exists"),
        "Actual error: {message}"
    );
    assert!(
        !message.to_lowercase().contains("unique")
            && !message.to_lowercase().contains("constraint"),
        "Raw database error leaked to client: {message}"
    );

    // Deleting twice: the second delete reports not-found instead of a
    // phantom success.
    let delete_mutation = r#"
        mutation { deleteRole(slug: "editor") { success slug } }
    "#;
    let res = schema.execute(delete_mutation).await;
    assert!(res.errors.is_empty(), "Errors: {:?}", res.errors);
    let res = schema.execute(delete_mutation).await;
    assert!(!res.errors.is_empty(), "Second delete must fail");
    assert!(
        res.errors[0].message.contains("not found"),
        "Actual error: {}",
        res.errors[0].message
    );

    // Updating a role that does not exist reports not-found.
    let update_mutation = r#"
        mutation {
            updateRole(input: { slug: "editor", name: "Ghost" }) { success }
        }
    "#;
    let res = schema.execute(update_mutation).await;
    assert!(!res.errors.is_empty(), "Updating a missing role must fail");
    assert!(
        res.errors[0].message.contains("not found"),
        "Actual error: {}",
        res.errors[0].message
    );
}

struct RecordingRoleWriter {
    slugs: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl rustok_rbac::graphql::RbacGraphqlRoleWriter for RecordingRoleWriter {
    async fn replace_user_role(
        &self,
        _tenant_id: &Uuid,
        _actor_id: &Uuid,
        _user_id: &Uuid,
        _role: rustok_core::UserRole,
    ) -> Result<(), rustok_rbac::graphql::RbacGraphqlRoleWriteError> {
        Ok(())
    }

    async fn replace_user_role_by_slug(
        &self,
        _tenant_id: &Uuid,
        _actor_id: &Uuid,
        _user_id: &Uuid,
        role_slug: &str,
    ) -> Result<(), rustok_rbac::graphql::RbacGraphqlRoleWriteError> {
        self.slugs
            .lock()
            .expect("writer lock")
            .push(role_slug.to_string());
        Ok(())
    }
}

#[tokio::test]
async fn test_assign_user_role_accepts_custom_role_slug() {
    let db = setup_test_db().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let target_user = Uuid::new_v4();
    let slugs = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let auth = AuthContext {
        user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![Permission::USERS_MANAGE],
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    };
    let tenant = TenantContext {
        id: tenant_id,
        name: "Test Tenant".to_string(),
        slug: "test-tenant".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let schema = Schema::build(RbacQuery, RbacMutation, EmptySubscription)
        .data(db)
        .data(auth)
        .data(AuthPrincipalContext::new(AuthPrincipalKind::DirectUser))
        .data(tenant)
        .data(rustok_rbac::graphql::RbacGraphqlRoleWriterHandle(
            std::sync::Arc::new(RecordingRoleWriter {
                slugs: slugs.clone(),
            }),
        ))
        .finish();

    // Custom role slug assignment flows through the slug writer method.
    let mutation = format!(
        r#"mutation {{ assignUserRole(input: {{ userId: "{target_user}", roleSlug: "content_editor" }}) {{ success userId role }} }}"#
    );
    let res = schema.execute(mutation).await;
    assert!(res.errors.is_empty(), "Errors: {:?}", res.errors);
    let data = res.data.into_json().expect("json data");
    assert_eq!(data["assignUserRole"]["role"], "content_editor");
    assert_eq!(
        slugs.lock().expect("writer lock").as_slice(),
        ["content_editor"]
    );

    // The legacy enum field still works and resolves to the same slug path.
    let mutation = format!(
        r#"mutation {{ assignUserRole(input: {{ userId: "{target_user}", role: ADMIN }}) {{ success role }} }}"#
    );
    let res = schema.execute(mutation).await;
    assert!(res.errors.is_empty(), "Errors: {:?}", res.errors);
    assert_eq!(
        slugs.lock().expect("writer lock").as_slice(),
        ["content_editor", "admin"]
    );

    // Conflicting enum/slug pairs and a missing role reference are rejected.
    let mutation = format!(
        r#"mutation {{ assignUserRole(input: {{ userId: "{target_user}", role: ADMIN, roleSlug: "manager" }}) {{ success }} }}"#
    );
    let res = schema.execute(mutation).await;
    assert!(!res.errors.is_empty(), "Conflicting role fields must fail");

    let mutation = format!(
        r#"mutation {{ assignUserRole(input: {{ userId: "{target_user}" }}) {{ success }} }}"#
    );
    let res = schema.execute(mutation).await;
    assert!(!res.errors.is_empty(), "Missing role reference must fail");
}

struct DisabledBlogAvailability;

#[async_trait::async_trait]
impl rustok_rbac::graphql::RbacModuleAvailabilityProvider for DisabledBlogAvailability {
    async fn permission_availability(
        &self,
        _tenant_id: &Uuid,
    ) -> Option<rustok_rbac::PermissionAvailability> {
        Some(rustok_rbac::PermissionAvailability::new(
            std::collections::BTreeMap::from([
                ("posts:read".to_string(), "blog".to_string()),
                ("posts:create".to_string(), "blog".to_string()),
            ]),
            std::collections::BTreeSet::new(),
        ))
    }
}

#[tokio::test]
async fn test_disabled_module_permissions_are_annotated_and_not_grantable() {
    let db = setup_test_db().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let auth = AuthContext {
        user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![
            Permission::SETTINGS_READ,
            Permission::SETTINGS_MANAGE,
            Permission::POSTS_READ,
        ],
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    };
    let tenant = TenantContext {
        id: tenant_id,
        name: "Test Tenant".to_string(),
        slug: "test-tenant".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let schema = Schema::build(RbacQuery, RbacMutation, EmptySubscription)
        .data(db)
        .data(auth)
        .data(AuthPrincipalContext::new(AuthPrincipalKind::DirectUser))
        .data(tenant)
        .data(rustok_rbac::graphql::RbacModuleAvailabilityHandle(
            std::sync::Arc::new(DisabledBlogAvailability),
        ))
        .finish();

    // The catalog annotates module ownership and tenant availability.
    let res = schema
        .execute("query { platformPermissions { id moduleSlug moduleStatus } }")
        .await;
    assert!(res.errors.is_empty(), "Errors: {:?}", res.errors);
    let data = res.data.into_json().expect("json data");
    let items = data["platformPermissions"].as_array().expect("items");
    let posts_read = items
        .iter()
        .find(|item| item["id"] == "posts:read")
        .expect("posts:read in catalog");
    assert_eq!(posts_read["moduleSlug"], "blog");
    assert_eq!(posts_read["moduleStatus"], "disabled");
    let users_read = items
        .iter()
        .find(|item| item["id"] == "users:read")
        .expect("users:read in catalog");
    assert_eq!(users_read["moduleStatus"], "core");

    // Granting a dormant permission is refused with a clear client error.
    let res = schema
        .execute(
            r#"mutation { createRole(input: { name: "Blog Editor", slug: "blog_editor", permissions: ["posts:read"] }) { success } }"#,
        )
        .await;
    assert!(!res.errors.is_empty(), "Dormant grant must fail");
    assert!(
        res.errors[0].message.contains("disabled"),
        "Actual error: {}",
        res.errors[0].message
    );
}

struct RecordingGrantNotifier {
    db: DatabaseConnection,
    observations: std::sync::Arc<std::sync::Mutex<Vec<(u64, u64)>>>,
    fail: bool,
}

#[async_trait::async_trait]
impl rustok_rbac::graphql::RbacGraphqlRoleWriter for RecordingGrantNotifier {
    async fn replace_user_role(
        &self,
        _: &Uuid,
        _: &Uuid,
        _: &Uuid,
        _: rustok_core::UserRole,
    ) -> Result<(), rustok_rbac::graphql::RbacGraphqlRoleWriteError> {
        unreachable!("test only edits role grants")
    }

    async fn role_permissions_committed(
        &self,
        _: &Uuid,
        _: &Uuid,
        generation: u64,
    ) -> Result<(), rustok_rbac::graphql::RbacGraphqlRoleWriteError> {
        let persisted = rustok_rbac::read_permission_invalidation_generation(&self.db)
            .await
            .map_err(|error| {
                rustok_rbac::graphql::RbacGraphqlRoleWriteError::Internal(error.to_string())
            })?;
        self.observations
            .lock()
            .unwrap()
            .push((generation, persisted));
        if self.fail {
            Err(rustok_rbac::graphql::RbacGraphqlRoleWriteError::Internal(
                "simulated failure".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

async fn create_editor(schema: &Schema<RbacQuery, RbacMutation, EmptySubscription>) {
    let response = schema.execute(r#"
        mutation { createRole(input: {name: "Editor", slug: "editor", permissions: ["posts:read"]}) { success } }
    "#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
}

async fn editor_state(db: &DatabaseConnection) -> (String, Vec<String>) {
    let row = db
        .query_one_raw(sea_orm::Statement::from_string(
            sea_orm::DbBackend::Sqlite,
            "SELECT name FROM roles WHERE slug = 'editor'".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let name: String = row.try_get("", "name").unwrap();
    let rows = db.query_all_raw(sea_orm::Statement::from_string(
        sea_orm::DbBackend::Sqlite,
        "SELECT p.resource, p.action FROM roles r JOIN role_permissions rp ON rp.role_id = r.id JOIN permissions p ON p.id = rp.permission_id WHERE r.slug = 'editor' ORDER BY p.resource, p.action".to_string(),
    )).await.unwrap();
    let grants = rows
        .into_iter()
        .map(|row| {
            format!(
                "{}:{}",
                row.try_get::<String>("", "resource").unwrap(),
                row.try_get::<String>("", "action").unwrap()
            )
        })
        .collect();
    (name, grants)
}

#[tokio::test]
async fn role_grant_edit_commits_generation_and_notifies_after_commit() {
    use rustok_rbac::graphql::RbacGraphqlRoleWriterHandle;
    let db = setup_test_db().await;
    let schema = test_schema(
        db.clone(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        vec![Permission::SETTINGS_MANAGE, Permission::POSTS_MANAGE],
    );
    create_editor(&schema).await;
    let observations = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = RbacGraphqlRoleWriterHandle(std::sync::Arc::new(RecordingGrantNotifier {
        db: db.clone(),
        observations: observations.clone(),
        fail: false,
    }));

    let metadata = schema
        .execute(
            async_graphql::Request::new(
                "mutation { updateRole(input: {slug: \"editor\", name: \"Senior\"}) { success } }",
            )
            .data(writer.clone()),
        )
        .await;
    assert!(metadata.errors.is_empty(), "{:?}", metadata.errors);
    assert_eq!(
        rustok_rbac::read_permission_invalidation_generation(&db)
            .await
            .unwrap(),
        0
    );
    assert!(observations.lock().unwrap().is_empty());

    let update = "mutation { updateRole(input: {slug: \"editor\", name: \"Changed\", permissions: [\"posts:update\"]}) { success } }";
    let response = schema
        .execute(async_graphql::Request::new(update).data(writer.clone()))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        editor_state(&db).await,
        ("Changed".to_string(), vec!["posts:update".to_string()])
    );
    assert_eq!(*observations.lock().unwrap(), vec![(1, 1)]);
    assert_eq!(
        rustok_rbac::read_permission_invalidation_generation(&db)
            .await
            .unwrap(),
        1
    );

    // The current implementation rewrites even an identical set; this must
    // still publish a fresh generation rather than pretending it was a no-op.
    let repeated = schema
        .execute(async_graphql::Request::new(update).data(writer))
        .await;
    assert!(repeated.errors.is_empty(), "{:?}", repeated.errors);
    assert_eq!(*observations.lock().unwrap(), vec![(1, 1), (2, 2)]);
}

#[tokio::test]
async fn role_grant_edit_rolls_back_if_generation_is_missing_or_exhausted() {
    for exhausted in [false, true] {
        let db = setup_test_db().await;
        let schema = test_schema(
            db.clone(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            vec![Permission::SETTINGS_MANAGE, Permission::POSTS_MANAGE],
        );
        create_editor(&schema).await;
        let observations = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let writer = rustok_rbac::graphql::RbacGraphqlRoleWriterHandle(std::sync::Arc::new(
            RecordingGrantNotifier {
                db: db.clone(),
                observations: observations.clone(),
                fail: false,
            },
        ));
        if exhausted {
            db.execute_unprepared("UPDATE rbac_invalidation_state SET generation = 9223372036854775807 WHERE scope = 'permissions'").await.unwrap();
        } else {
            db.execute_unprepared(
                "DELETE FROM rbac_invalidation_state WHERE scope = 'permissions'",
            )
            .await
            .unwrap();
        }
        let response = schema.execute(async_graphql::Request::new("mutation { updateRole(input: {slug: \"editor\", name: \"Must Roll Back\", permissions: [\"posts:update\"]}) { success } }").data(writer)).await;
        assert!(!response.errors.is_empty());
        assert!(observations.lock().unwrap().is_empty());
        assert!(
            !response.errors[0]
                .message
                .contains("rbac_invalidation_state")
        );
        assert_eq!(
            editor_state(&db).await,
            ("Editor".to_string(), vec!["posts:read".to_string()])
        );
        if exhausted {
            assert_eq!(
                rustok_rbac::read_permission_invalidation_generation(&db)
                    .await
                    .unwrap(),
                i64::MAX as u64
            );
        } else {
            assert!(
                rustok_rbac::read_permission_invalidation_generation(&db)
                    .await
                    .is_err()
            );
        }
    }
}

#[tokio::test]
async fn role_grant_write_failure_rolls_back_without_advancing_generation() {
    let db = setup_test_db().await;
    let schema = test_schema(
        db.clone(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        vec![Permission::SETTINGS_MANAGE, Permission::POSTS_MANAGE],
    );
    create_editor(&schema).await;
    db.execute_unprepared(
        "CREATE TRIGGER reject_role_grant BEFORE INSERT ON role_permissions BEGIN SELECT RAISE(ABORT, 'simulated grant write failure'); END",
    ).await.unwrap();
    let response = schema.execute(
        "mutation { updateRole(input: {slug: \"editor\", name: \"Must Roll Back\", permissions: [\"posts:update\"]}) { success } }",
    ).await;
    assert!(!response.errors.is_empty());
    assert!(
        !response.errors[0]
            .message
            .contains("simulated grant write failure")
    );
    assert_eq!(
        editor_state(&db).await,
        ("Editor".to_string(), vec!["posts:read".to_string()])
    );
    assert_eq!(
        rustok_rbac::read_permission_invalidation_generation(&db)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn role_grant_edit_callback_failure_does_not_undo_commit() {
    use rustok_rbac::graphql::RbacGraphqlRoleWriterHandle;
    let db = setup_test_db().await;
    let schema = test_schema(
        db.clone(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        vec![Permission::SETTINGS_MANAGE, Permission::POSTS_MANAGE],
    );
    create_editor(&schema).await;
    let observations = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = RbacGraphqlRoleWriterHandle(std::sync::Arc::new(RecordingGrantNotifier {
        db: db.clone(),
        observations: observations.clone(),
        fail: true,
    }));
    let response = schema.execute(async_graphql::Request::new(
        "mutation { updateRole(input: {slug: \"editor\", permissions: [\"posts:update\"]}) { success } }",
    ).data(writer)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["updateRole"]["success"],
        true
    );
    assert_eq!(*observations.lock().unwrap(), vec![(1, 1), (1, 1)]);
    assert_eq!(editor_state(&db).await.1, vec!["posts:update".to_string()]);
    assert_eq!(
        rustok_rbac::read_permission_invalidation_generation(&db)
            .await
            .unwrap(),
        1
    );
}

struct LegacyInvalidationNotifier(std::sync::Arc<std::sync::atomic::AtomicUsize>);

#[async_trait::async_trait]
impl rustok_rbac::graphql::RbacGraphqlRoleWriter for LegacyInvalidationNotifier {
    async fn replace_user_role(
        &self,
        _: &Uuid,
        _: &Uuid,
        _: &Uuid,
        _: rustok_core::UserRole,
    ) -> Result<(), rustok_rbac::graphql::RbacGraphqlRoleWriteError> {
        Ok(())
    }
    async fn invalidate_role_users(
        &self,
        _: &Uuid,
        _: &Uuid,
    ) -> Result<(), rustok_rbac::graphql::RbacGraphqlRoleWriteError> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn generation_callback_defaults_to_legacy_membership_hook() {
    use rustok_rbac::graphql::RbacGraphqlRoleWriter;
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let writer = LegacyInvalidationNotifier(calls.clone());
    writer
        .role_permissions_committed(&Uuid::new_v4(), &Uuid::new_v4(), 42)
        .await
        .unwrap();
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}
