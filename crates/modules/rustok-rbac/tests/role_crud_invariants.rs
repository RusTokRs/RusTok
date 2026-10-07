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
        slug: "test-tenant".to_string(),
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
        vec![Permission::SETTINGS_READ, Permission::SETTINGS_MANAGE],
    );

    // Seed system role super_admin
    db.execute_unprepared(&format!(
        "INSERT INTO roles (id, tenant_id, name, slug, description, is_system) VALUES ('{}', '{}', 'Super Admin', 'super_admin', 'Built-in', 1)",
        Uuid::new_v4(), tenant_id
    )).await.expect("insert system role");

    // Try to delete system role -> MUST FAIL
    let del_system_mutation = r#"
        mutation {
            deleteRole(slug: "super_admin") {
                success
                slug
            }
        }
    "#;
    let res = schema.execute(del_system_mutation).await;
    assert!(!res.errors.is_empty(), "Deleting system role must fail");
    assert!(
        res.errors[0]
            .message
            .contains("Cannot delete built-in system role")
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
            .contains("Super administrator permissions are immutable")
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
    let custom_role_id = role["id"].as_str().expect("role id");
    db.execute_unprepared(&format!(
        "INSERT INTO user_roles (id, user_id, role_id) VALUES ('{}', '{}', '{}')",
        Uuid::new_v4(),
        target_user,
        custom_role_id
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
            .contains("currently assigned to 1 user(s)")
    );

    // 5. Unassign user and delete custom role -> MUST SUCCEED
    db.execute_unprepared(&format!(
        "DELETE FROM user_roles WHERE role_id = '{}'",
        custom_role_id
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
