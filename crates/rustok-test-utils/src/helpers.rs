//! Helper functions for testing
//!
//! Provides utility functions for common testing scenarios.

use rustok_core::{SecurityContext, UserRole};
use uuid::Uuid;

/// Creates a security context for a super admin user.
///
/// # Example
///
/// ```rust
/// use rustok_core::UserRole;
/// use rustok_test_utils::helpers::super_admin_context;
///
/// let ctx = super_admin_context();
/// assert!(matches!(ctx.role, UserRole::SuperAdmin));
/// ```
pub fn super_admin_context() -> SecurityContext {
    SecurityContext::system()
}

/// Creates a security context for an admin user.
///
/// # Example
///
/// ```rust
/// use rustok_core::UserRole;
/// use rustok_test_utils::helpers::admin_context;
///
/// let ctx = admin_context();
/// assert!(matches!(ctx.role, UserRole::Admin));
/// ```
pub fn admin_context() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

/// Creates a security context for a manager user.
///
/// # Example
///
/// ```rust
/// use rustok_core::UserRole;
/// use rustok_test_utils::helpers::manager_context;
///
/// let ctx = manager_context();
/// assert!(matches!(ctx.role, UserRole::Manager));
/// ```
pub fn manager_context() -> SecurityContext {
    SecurityContext::new(UserRole::Manager, Some(Uuid::new_v4()))
}

/// Creates a security context for a customer user.
///
/// # Example
///
/// ```rust
/// use rustok_core::UserRole;
/// use rustok_test_utils::helpers::customer_context;
///
/// let ctx = customer_context();
/// assert!(matches!(ctx.role, UserRole::Customer));
/// ```
pub fn customer_context() -> SecurityContext {
    SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()))
}

/// Creates a security context for a specific user ID with a given role.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::helpers::user_context;
/// use rustok_core::UserRole;
/// use uuid::Uuid;
///
/// let user_id = Uuid::new_v4();
/// let ctx = user_context(UserRole::Admin, user_id);
/// ```
pub fn user_context(role: UserRole, user_id: Uuid) -> SecurityContext {
    SecurityContext::new(role, Some(user_id))
}

/// Asserts that a result is an error of a specific type.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::assert_error_type;
///
/// fn may_fail() -> Result<i32, String> {
///     Err("error".to_string())
/// }
///
/// let result = may_fail();
/// assert_error_type!(result, String);
/// ```
#[macro_export]
macro_rules! assert_error_type {
    ($result:expr, $type:ty) => {
        match $result {
            Err(e) => {
                let _: &$type = &e;
            }
            Ok(_) => panic!("Expected error, got Ok"),
        }
    };
}

/// Asserts that a result is Ok and returns the value.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::assert_ok;
///
/// fn may_succeed() -> Result<i32, String> {
///     Ok(42)
/// }
///
/// let result = may_succeed();
/// let value = assert_ok!(result);
/// assert_eq!(value, 42);
/// ```
#[macro_export]
macro_rules! assert_ok {
    ($result:expr) => {
        match $result {
            Ok(v) => v,
            Err(e) => panic!("Expected Ok, got Err: {:?}", e),
        }
    };
}

/// Asserts that a result is Err and returns the error.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::assert_err;
///
/// fn may_fail() -> Result<i32, String> {
///     Err("error".to_string())
/// }
///
/// let result = may_fail();
/// let error = assert_err!(result);
/// assert_eq!(error, "error");
/// ```
#[macro_export]
macro_rules! assert_err {
    ($result:expr) => {
        match $result {
            Err(e) => e,
            Ok(v) => panic!("Expected Err, got Ok: {:?}", v),
        }
    };
}

/// Waits for a condition to be true with a timeout.
///
/// This is useful for async tests that need to wait for some condition.
///
/// # Example
///
/// ```rust,ignore
/// use rustok_test_utils::helpers::wait_for;
///
/// #[tokio::test]
/// async fn test_async_condition() {
///     let flag = Arc::new(AtomicBool::new(false));
///     let flag_clone = flag.clone();
///
///     tokio::spawn(async move {
///         tokio::time::sleep(Duration::from_millis(50)).await;
///         flag_clone.store(true, Ordering::SeqCst);
///     });
///
///     wait_for(|| flag.load(Ordering::SeqCst), Duration::from_secs(1)).await;
/// }
/// ```
pub async fn wait_for<F>(condition: F, timeout: std::time::Duration)
where
    F: Fn() -> bool,
{
    let start = std::time::Instant::now();
    let check_interval = std::time::Duration::from_millis(10);

    while !condition() {
        if start.elapsed() > timeout {
            panic!("Timeout waiting for condition");
        }
        tokio::time::sleep(check_interval).await;
    }
}

/// Generates a unique test ID with a prefix.
///
/// This is useful for creating unique identifiers in tests.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::helpers::unique_test_id;
///
/// let id = unique_test_id("product");
/// // id might be "product-550e8400-e29b-41d4-a716-446655440000"
/// ```
pub fn unique_test_id(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}

/// Generates a unique email address for testing.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::helpers::unique_email;
///
/// let email = unique_email();
/// // email might be "test-550e8400@example.com"
/// ```
pub fn unique_email() -> String {
    format!(
        "test-{}@example.com",
        Uuid::new_v4().to_string().split('-').next().unwrap()
    )
}

/// Generates a unique slug for testing.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::helpers::unique_slug;
///
/// let slug = unique_slug("post");
/// // slug might be "post-550e8400"
/// ```
pub fn unique_slug(prefix: &str) -> String {
    format!(
        "{}-{}",
        prefix,
        Uuid::new_v4().to_string().split('-').next().unwrap()
    )
}

/// Creates a test JSON payload.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::helpers::json_payload;
/// use serde_json::json;
///
/// let payload = json_payload(json!({
///     "name": "Test",
///     "value": 42
/// }));
/// ```
pub fn json_payload<T: serde::Serialize>(data: T) -> serde_json::Value {
    serde_json::to_value(data).expect("Failed to serialize to JSON")
}

/// Asserts that a permission scope is as expected.
///
/// # Example
///
/// ```rust
/// use rustok_core::PermissionScope;
/// use rustok_test_utils::assert_permission_scope;
///
/// let scope = PermissionScope::All;
/// assert_permission_scope!(scope, All);
/// ```
#[macro_export]
macro_rules! assert_permission_scope {
    ($scope:expr, $expected:ident) => {
        match $scope {
            PermissionScope::$expected => (),
            other => panic!(
                "Expected PermissionScope::{}, got {:?}",
                stringify!($expected),
                other
            ),
        }
    };
}

/// Runs a test with multiple security contexts.
///
/// This is useful for testing RBAC with different roles.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::helpers::with_roles;
/// use rustok_core::{UserRole, SecurityContext};
///
/// async fn test_access(ctx: SecurityContext) -> bool {
///     // Test logic here
///     true
/// }
///
/// #[tokio::test]
/// async fn test_with_multiple_roles() {
///     with_roles(&[UserRole::Admin, UserRole::Customer], |ctx| async move {
///         assert!(test_access(ctx).await);
///     }).await;
/// }
/// ```
pub async fn with_roles<F, Fut>(roles: &[UserRole], test: F)
where
    F: Fn(SecurityContext) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    for role in roles {
        let ctx = SecurityContext::new(role.clone(), Some(Uuid::new_v4()));
        test(ctx).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_context_helpers() {
        let super_admin = super_admin_context();
        assert!(matches!(super_admin.role, UserRole::SuperAdmin));

        let admin = admin_context();
        assert!(matches!(admin.role, UserRole::Admin));

        let manager = manager_context();
        assert!(matches!(manager.role, UserRole::Manager));

        let customer = customer_context();
        assert!(matches!(customer.role, UserRole::Customer));
    }

    #[test]
    fn test_unique_test_id() {
        let id1 = unique_test_id("test");
        let id2 = unique_test_id("test");
        assert_ne!(id1, id2);
        assert!(id1.starts_with("test-"));
    }

    #[test]
    fn test_unique_email() {
        let email1 = unique_email();
        let email2 = unique_email();
        assert_ne!(email1, email2);
        assert!(email1.ends_with("@example.com"));
    }

    #[test]
    fn test_unique_slug() {
        let slug = unique_slug("post");
        assert!(slug.starts_with("post-"));
    }

    #[tokio::test]
    async fn test_wait_for() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            flag_clone.store(true, Ordering::SeqCst);
        });

        wait_for(|| flag.load(Ordering::SeqCst), Duration::from_secs(1)).await;

        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_assert_ok_macro() {
        let result: Result<i32, ()> = Ok(42);
        let value = assert_ok!(result);
        assert_eq!(value, 42);
    }

    #[test]
    fn test_assert_err_macro() {
        let result: Result<(), String> = Err("error".to_string());
        let error = assert_err!(result);
        assert_eq!(error, "error");
    }
}
