use axum::http::StatusCode;
use rustok_core::Permission;

use crate::extractors::auth::CurrentUser;

fn has_effective_permission(user: &CurrentUser, permission: &Permission) -> bool {
    rustok_rbac::has_effective_permission_in_set(&user.permissions, permission)
}

/// Extractor that enforces a specific permission
///
/// This extractor will:
/// 1. Extract the CurrentUser (which handles authentication)
/// 2. Check if the user has the required permission
/// 3. Return 403 Forbidden if permission is denied
///
/// # Example
/// ```rust,ignore
/// async fn create_product(
///     State(ctx): State<AppContext>,
///     RequirePermission(user, Permission::PRODUCTS_CREATE): RequirePermission,
///     Json(input): Json<CreateProductInput>,
/// ) -> Result<Json<ProductResponse>> {
///     // User is guaranteed to have PRODUCTS_CREATE permission
///     // ...
/// }
/// ```
/// Helper macro to create permission-checking extractors
///
/// Since Rust doesn't support const generics for complex types yet,
/// we need to create wrapper types for each permission check.
///
/// # Example
/// ```rust,ignore
/// // In controller
/// async fn create_product(
///     State(ctx): State<AppContext>,
///     RequireProductCreate(user): RequireProductCreate,
///     Json(input): Json<CreateProductInput>,
/// ) -> Result<Json<ProductResponse>> {
///     // User has PRODUCTS_CREATE permission
/// }
/// ```
#[macro_export]
macro_rules! define_permission_extractor {
    ($name:ident, $permission:expr) => {
        pub struct $name(pub $crate::extractors::auth::CurrentUser);

        impl<S> axum::extract::FromRequestParts<S> for $name
        where
            S: Send + Sync,
            loco_rs::app::AppContext: axum::extract::FromRef<S>,
        {
            type Rejection = (axum::http::StatusCode, String);

            async fn from_request_parts(
                parts: &mut axum::http::request::Parts,
                state: &S,
            ) -> Result<Self, Self::Rejection> {
                let user = $crate::extractors::auth::CurrentUser::from_request_parts(parts, state)
                    .await
                    .map_err(|(code, msg)| (code, msg.to_string()))?;

                if !has_effective_permission(&user, &$permission) {
                    return Err((
                        axum::http::StatusCode::FORBIDDEN,
                        format!("Insufficient permissions. Required: {}", $permission),
                    ));
                }

                Ok($name(user))
            }
        }
    };
}

// Define common permission extractors
define_permission_extractor!(RequirePostsCreate, rustok_core::Permission::POSTS_CREATE);
define_permission_extractor!(RequirePostsRead, rustok_core::Permission::POSTS_READ);
define_permission_extractor!(RequirePostsUpdate, rustok_core::Permission::POSTS_UPDATE);
define_permission_extractor!(RequirePostsDelete, rustok_core::Permission::POSTS_DELETE);
define_permission_extractor!(RequirePostsList, rustok_core::Permission::POSTS_LIST);

define_permission_extractor!(RequireNodesCreate, rustok_core::Permission::NODES_CREATE);
define_permission_extractor!(RequireNodesRead, rustok_core::Permission::NODES_READ);
define_permission_extractor!(RequireNodesUpdate, rustok_core::Permission::NODES_UPDATE);
define_permission_extractor!(RequireNodesDelete, rustok_core::Permission::NODES_DELETE);
define_permission_extractor!(RequireNodesList, rustok_core::Permission::NODES_LIST);

define_permission_extractor!(
    RequireProductsCreate,
    rustok_core::Permission::PRODUCTS_CREATE
);
define_permission_extractor!(RequireProductsRead, rustok_core::Permission::PRODUCTS_READ);
define_permission_extractor!(
    RequireProductsUpdate,
    rustok_core::Permission::PRODUCTS_UPDATE
);
define_permission_extractor!(
    RequireProductsDelete,
    rustok_core::Permission::PRODUCTS_DELETE
);
define_permission_extractor!(RequireProductsList, rustok_core::Permission::PRODUCTS_LIST);

define_permission_extractor!(RequireOrdersCreate, rustok_core::Permission::ORDERS_CREATE);
define_permission_extractor!(RequireOrdersRead, rustok_core::Permission::ORDERS_READ);
define_permission_extractor!(RequireOrdersUpdate, rustok_core::Permission::ORDERS_UPDATE);
define_permission_extractor!(RequireOrdersDelete, rustok_core::Permission::ORDERS_DELETE);
define_permission_extractor!(RequireOrdersList, rustok_core::Permission::ORDERS_LIST);

define_permission_extractor!(RequireUsersCreate, rustok_core::Permission::USERS_CREATE);
define_permission_extractor!(RequireUsersRead, rustok_core::Permission::USERS_READ);
define_permission_extractor!(RequireUsersUpdate, rustok_core::Permission::USERS_UPDATE);
define_permission_extractor!(RequireUsersDelete, rustok_core::Permission::USERS_DELETE);
define_permission_extractor!(RequireUsersList, rustok_core::Permission::USERS_LIST);

define_permission_extractor!(RequireSettingsRead, rustok_core::Permission::SETTINGS_READ);
define_permission_extractor!(
    RequireSettingsUpdate,
    rustok_core::Permission::SETTINGS_UPDATE
);

define_permission_extractor!(
    RequireAnalyticsRead,
    rustok_core::Permission::ANALYTICS_READ
);
define_permission_extractor!(
    RequireAnalyticsExport,
    rustok_core::Permission::ANALYTICS_EXPORT
);

define_permission_extractor!(
    RequireBlogPostsCreate,
    rustok_core::Permission::BLOG_POSTS_CREATE
);
define_permission_extractor!(
    RequireBlogPostsRead,
    rustok_core::Permission::BLOG_POSTS_READ
);
define_permission_extractor!(
    RequireBlogPostsUpdate,
    rustok_core::Permission::BLOG_POSTS_UPDATE
);
define_permission_extractor!(
    RequireBlogPostsDelete,
    rustok_core::Permission::BLOG_POSTS_DELETE
);
define_permission_extractor!(
    RequireBlogPostsList,
    rustok_core::Permission::BLOG_POSTS_LIST
);
define_permission_extractor!(
    RequireBlogPostsPublish,
    rustok_core::Permission::BLOG_POSTS_PUBLISH
);

define_permission_extractor!(
    RequireForumTopicsCreate,
    rustok_core::Permission::FORUM_TOPICS_CREATE
);
define_permission_extractor!(
    RequireForumTopicsRead,
    rustok_core::Permission::FORUM_TOPICS_READ
);
define_permission_extractor!(
    RequireForumTopicsUpdate,
    rustok_core::Permission::FORUM_TOPICS_UPDATE
);
define_permission_extractor!(
    RequireForumTopicsDelete,
    rustok_core::Permission::FORUM_TOPICS_DELETE
);
define_permission_extractor!(
    RequireForumTopicsList,
    rustok_core::Permission::FORUM_TOPICS_LIST
);
define_permission_extractor!(
    RequireForumTopicsModerate,
    rustok_core::Permission::FORUM_TOPICS_MODERATE
);

define_permission_extractor!(
    RequireForumRepliesCreate,
    rustok_core::Permission::FORUM_REPLIES_CREATE
);
define_permission_extractor!(
    RequireForumRepliesRead,
    rustok_core::Permission::FORUM_REPLIES_READ
);
define_permission_extractor!(
    RequireForumRepliesModerate,
    rustok_core::Permission::FORUM_REPLIES_MODERATE
);

define_permission_extractor!(
    RequireForumCategoriesCreate,
    rustok_core::Permission::FORUM_CATEGORIES_CREATE
);
define_permission_extractor!(
    RequireForumCategoriesList,
    rustok_core::Permission::FORUM_CATEGORIES_LIST
);
define_permission_extractor!(
    RequireForumCategoriesUpdate,
    rustok_core::Permission::FORUM_CATEGORIES_UPDATE
);
define_permission_extractor!(
    RequireForumCategoriesDelete,
    rustok_core::Permission::FORUM_CATEGORIES_DELETE
);

define_permission_extractor!(RequirePagesCreate, rustok_core::Permission::PAGES_CREATE);
define_permission_extractor!(RequirePagesRead, rustok_core::Permission::PAGES_READ);
define_permission_extractor!(RequirePagesUpdate, rustok_core::Permission::PAGES_UPDATE);
define_permission_extractor!(RequirePagesDelete, rustok_core::Permission::PAGES_DELETE);

define_permission_extractor!(
    RequireInventoryRead,
    rustok_core::Permission::INVENTORY_READ
);
define_permission_extractor!(
    RequireInventoryUpdate,
    rustok_core::Permission::INVENTORY_UPDATE
);
define_permission_extractor!(
    RequireInventoryList,
    rustok_core::Permission::INVENTORY_LIST
);

define_permission_extractor!(
    RequireScriptsCreate,
    rustok_core::Permission::SCRIPTS_CREATE
);
define_permission_extractor!(RequireScriptsRead, rustok_core::Permission::SCRIPTS_READ);
define_permission_extractor!(RequireScriptsList, rustok_core::Permission::SCRIPTS_LIST);
define_permission_extractor!(
    RequireScriptsManage,
    rustok_core::Permission::SCRIPTS_MANAGE
);

define_permission_extractor!(
    RequireLogsRead,
    rustok_core::Permission::new(rustok_core::Resource::Logs, rustok_core::Action::Read)
);

/// Helper to check permission inline without extractor
///
/// # Example
/// ```rust,ignore
/// pub async fn some_handler(
///     user: CurrentUser,
///     // ...
/// ) -> Result<Response> {
///     check_permission(&user, Permission::PRODUCTS_UPDATE)?;
///     // ...
/// }
/// ```
pub fn check_permission(
    user: &CurrentUser,
    permission: Permission,
) -> Result<(), (StatusCode, String)> {
    if !has_effective_permission(user, &permission) {
        return Err((
            StatusCode::FORBIDDEN,
            format!("Insufficient permissions. Required: {}", permission),
        ));
    }
    Ok(())
}

/// Helper to check if user has any of the provided permissions
pub fn check_any_permission(
    user: &CurrentUser,
    permissions: &[Permission],
) -> Result<(), (StatusCode, String)> {
    if !permissions
        .iter()
        .any(|permission| has_effective_permission(user, permission))
    {
        let perms_str = permissions
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err((
            StatusCode::FORBIDDEN,
            format!("Insufficient permissions. Required any of: {}", perms_str),
        ));
    }
    Ok(())
}

/// Helper to check if user has all of the provided permissions
pub fn check_all_permissions(
    user: &CurrentUser,
    permissions: &[Permission],
) -> Result<(), (StatusCode, String)> {
    if !permissions
        .iter()
        .all(|permission| has_effective_permission(user, permission))
    {
        let perms_str = permissions
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err((
            StatusCode::FORBIDDEN,
            format!("Insufficient permissions. Required all of: {}", perms_str),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use rustok_core::{Permission, UserRole};

    fn create_test_user(role: UserRole) -> CurrentUser {
        use crate::models::users;
        let user = users::Model {
            id: rustok_core::generate_id(),
            tenant_id: rustok_core::generate_id(),
            email: "test@example.com".to_string(),
            password_hash: "hash".to_string(),
            name: None,
            status: rustok_core::UserStatus::Active,
            email_verified_at: None,
            last_login_at: None,
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        };

        let permissions = rustok_core::Rbac::permissions_for_role(&role)
            .iter()
            .cloned()
            .collect();

        CurrentUser {
            user,
            session_id: rustok_core::generate_id(),
            permissions,
            inferred_role: role,
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        }
    }

    #[test]
    fn test_check_permission_allows_authorized() {
        let admin_user = create_test_user(UserRole::Admin);
        let result = check_permission(&admin_user, Permission::POSTS_CREATE);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_permission_denies_unauthorized() {
        let customer_user = create_test_user(UserRole::Customer);
        let result = check_permission(&customer_user, Permission::PRODUCTS_DELETE);
        assert!(result.is_err());
        if let Err((code, _msg)) = result {
            assert_eq!(code, StatusCode::FORBIDDEN);
        }
    }

    #[test]
    fn test_check_any_permission_success() {
        let customer_user = create_test_user(UserRole::Customer);
        let result = check_any_permission(
            &customer_user,
            &[Permission::POSTS_READ, Permission::PRODUCTS_CREATE],
        );
        assert!(result.is_ok()); // Customer has POSTS_READ
    }

    #[test]
    fn test_check_any_permission_failure() {
        let customer_user = create_test_user(UserRole::Customer);
        let result = check_any_permission(
            &customer_user,
            &[Permission::PRODUCTS_DELETE, Permission::USERS_DELETE],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_check_all_permissions_success() {
        let manager_user = create_test_user(UserRole::Manager);
        let result = check_all_permissions(
            &manager_user,
            &[Permission::PRODUCTS_READ, Permission::PRODUCTS_CREATE],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_all_permissions_failure() {
        let customer_user = create_test_user(UserRole::Customer);
        let result = check_all_permissions(
            &customer_user,
            &[Permission::PRODUCTS_READ, Permission::PRODUCTS_DELETE],
        );
        assert!(result.is_err()); // Customer doesn't have DELETE
    }
}
