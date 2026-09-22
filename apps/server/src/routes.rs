//! Shared Axum router state for the executable server host.

use crate::services::server_runtime_context::ServerAuthRuntime;

/// A router which still requires the host's auth/runtime state.
pub type ServerRouter = axum::Router<ServerAuthRuntime>;
