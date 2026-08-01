use rustok_core::ModuleRuntimeExtensions;

use crate::error::Result;
use crate::services::server_runtime_context::ServerRuntimeContext;

use super::{keyring_schedule, keyring_schedule_guard, keyring_schedule_trigger};

pub(super) fn register_comments_provider_runtime(
    extensions: &mut ModuleRuntimeExtensions,
) -> std::result::Result<(), String> {
    if let Some(trigger) = extensions
        .get::<keyring_schedule_trigger::SharedCommentsTcpDelegationScheduleTrigger>()
        .cloned()
    {
        if extensions
            .get::<keyring_schedule::SharedCommentsTcpDelegationScheduleHandle>()
            .is_some()
        {
            return Err(
                "Comments TCP delegation schedule trigger and standalone schedule handle cannot be combined"
                    .to_string(),
            );
        }
        extensions.insert(trigger.schedule_handle());
    }
    keyring_schedule_guard::register_comments_provider_runtime(extensions)
}

pub(super) async fn start_comments_tcp_listener_if_enabled(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<()> {
    keyring_schedule_guard::start_comments_tcp_listener_if_enabled(runtime_ctx).await
}
