mod client;
pub mod components;
pub mod context;
pub mod hooks;
pub mod storage;
pub mod transport;

use rustok_ui_auth::{AuthError, AuthSession, AuthUser};

pub const ADMIN_TOKEN_KEY: &str = "rustok-admin-token";
pub const ADMIN_TENANT_KEY: &str = "rustok-admin-tenant";
pub const ADMIN_USER_KEY: &str = "rustok-admin-user";
pub const ADMIN_SESSION_KEY: &str = "rustok-admin-session";

pub use client::AuthorizedBrowserClient;
pub use components::{GuestRoute, ProtectedRoute, RequireAuth};
pub use context::{AuthContext, AuthProvider};
pub use hooks::{
    use_auth, use_auth_error, use_current_user, use_is_authenticated, use_is_loading,
    use_is_token_valid, use_session, use_tenant, use_token,
};
pub use storage::{ServerAuthSnapshot, provide_server_auth_snapshot};
