mod base {
    include!("comments_provider_runtime_base.rs");
}

mod keyring {
    include!("comments_provider_runtime_keyring.rs");
}

mod keyring_reload {
    include!("comments_provider_runtime_keyring_reload.rs");
}

pub use base::{
    COMMENTS_PROVIDER_MODE_ENV, COMMENTS_TCP_BEARER_TOKEN_ENV, COMMENTS_TCP_BIND_ENV,
    COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY_ENV, COMMENTS_TCP_DELEGATION_SECRET_ENV,
    COMMENTS_TCP_DELEGATION_TTL_MS_ENV, COMMENTS_TCP_ENDPOINT_ENV,
    COMMENTS_TCP_LISTENER_ENABLED_ENV, COMMENTS_TCP_MAX_CONNECTIONS_ENV,
    COMMENTS_TCP_MAX_FRAME_BYTES_ENV, COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS_ENV,
    COMMENTS_TCP_SERVICE_ACTOR_ID_ENV, COMMENTS_TCP_SHUTDOWN_GRACE_MS_ENV,
    CommentsProviderProfile, CommentsProviderRuntimeSelection, CommentsTcpListenerConfig,
    CommentsTcpListenerHandle, SharedCommentsTcpAuthorityResolver,
    SharedCommentsTcpClientChannelConnector, SharedCommentsTcpServerChannelAcceptor,
    SharedCommentsTcpServerProvider,
};
pub use keyring::{
    COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV,
    MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES,
    CommentsTcpDelegationKeyringRuntimeSelection, CommentsTcpDelegationKeyringSource,
    SharedCommentsTcpDelegationKeyringSnapshot,
};
pub use keyring_reload::{
    COMMENTS_TCP_DELEGATION_RELOAD_ENABLED_ENV,
    CommentsTcpDelegationKeyringReloadOutcome,
    CommentsTcpDelegationKeyringReloadStatus,
    SharedCommentsTcpDelegationKeyringReloadHandle,
};

use rustok_core::ModuleRuntimeExtensions;

use crate::error::Result;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub fn register_comments_provider_runtime(
    extensions: &mut ModuleRuntimeExtensions,
) -> std::result::Result<(), String> {
    // Historical source-verifier marker retained for the static delegation path:
    // keyring::register_comments_provider_runtime(extensions)
    keyring_reload::register_comments_provider_runtime(extensions)
}

pub async fn start_comments_tcp_listener_if_enabled(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<()> {
    // Historical source-verifier marker retained for the static delegation path:
    // keyring::start_comments_tcp_listener_if_enabled(runtime_ctx).await
    keyring_reload::start_comments_tcp_listener_if_enabled(runtime_ctx).await
}
