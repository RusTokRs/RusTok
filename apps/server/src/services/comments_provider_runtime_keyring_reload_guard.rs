use std::sync::Arc;

use rustok_core::ModuleRuntimeExtensions;

use crate::error::{Error, Result};
use crate::services::server_runtime_context::ServerRuntimeContext;

use super::{base, keyring_reload};

pub(super) async fn start_comments_tcp_listener_if_enabled(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<()> {
    let extensions = runtime_ctx.shared_get::<Arc<ModuleRuntimeExtensions>>();
    let reload_handle_configured = runtime_ctx
        .shared_get::<keyring_reload::SharedCommentsTcpDelegationKeyringReloadHandle>()
        .is_some()
        || extensions.as_ref().is_some_and(|values| {
            values
                .get::<keyring_reload::SharedCommentsTcpDelegationKeyringReloadHandle>()
                .is_some()
        });
    if !reload_handle_configured {
        return keyring_reload::start_comments_tcp_listener_if_enabled(runtime_ctx).await;
    }

    let Some(_) = base::CommentsTcpListenerConfig::from_environment().map_err(Error::BadRequest)?
    else {
        return Ok(());
    };
    let authority_override_configured = runtime_ctx
        .shared_get::<base::SharedCommentsTcpAuthorityResolver>()
        .is_some()
        || extensions.as_ref().is_some_and(|values| {
            values
                .get::<base::SharedCommentsTcpAuthorityResolver>()
                .is_some()
        });
    if authority_override_configured && !reloadable_client_is_active(extensions.as_deref()) {
        return Err(Error::BadRequest(
            "Comments TCP delegation reload handle is unused because an external listener authority override is configured and no built-in reloadable TCP client is active"
                .to_string(),
        ));
    }

    keyring_reload::start_comments_tcp_listener_if_enabled(runtime_ctx).await
}

fn reloadable_client_is_active(extensions: Option<&ModuleRuntimeExtensions>) -> bool {
    extensions
        .and_then(|values| values.get::<base::CommentsProviderRuntimeSelection>())
        .is_some_and(|selection| {
            matches!(
                selection.profile,
                base::CommentsProviderProfile::TcpLoopback
                    | base::CommentsProviderProfile::TcpProtectedLoopback
            )
        })
}
