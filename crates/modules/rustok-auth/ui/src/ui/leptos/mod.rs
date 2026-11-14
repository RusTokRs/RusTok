//! Leptos bindings for the module-owned Auth client.

mod client;
pub mod components;
pub mod context;
pub mod hooks;
mod realm;
pub mod storage;

pub use client::AuthorizedBrowserClient;
pub use components::{GuestRoute, ProtectedRoute, RequireAuth};
pub use context::{
    AUTH_SCOPE_CHANGE_EVENT, AUTH_SCOPE_CHANGE_STORAGE_KEY, AuthContext, AuthProvider,
};
pub use hooks::{
    use_auth, use_auth_error, use_current_user, use_is_authenticated, use_is_loading,
    use_is_token_valid, use_session, use_tenant, use_token,
};
pub use storage::{ServerAuthSnapshot, provide_server_auth_snapshot};
