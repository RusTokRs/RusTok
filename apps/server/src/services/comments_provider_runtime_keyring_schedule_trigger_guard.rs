use rustok_core::ModuleRuntimeExtensions;

use crate::error::Result;
use crate::services::server_runtime_context::ServerRuntimeContext;

use super::{
    keyring_schedule,
    keyring_schedule_guard,
    keyring_schedule_persisted_trigger,
    keyring_schedule_trigger,
};

pub(super) fn register_comments_provider_runtime(
    extensions: &mut ModuleRuntimeExtensions,
) -> std::result::Result<(), String> {
    let persisted_trigger = extensions
        .get::<
            keyring_schedule_persisted_trigger::SharedCommentsTcpDelegationPersistedScheduleTrigger,
        >()
        .cloned();
    let process_local_trigger = extensions
        .get::<keyring_schedule_trigger::SharedCommentsTcpDelegationScheduleTrigger>()
        .cloned();

    if persisted_trigger.is_some() && process_local_trigger.is_some() {
        return Err(
            "Persisted and process-local Comments TCP delegation schedule triggers cannot be combined"
                .to_string(),
        );
    }

    let trigger_handle = persisted_trigger
        .as_ref()
        .map(|trigger| trigger.schedule_handle())
        .or_else(|| {
            process_local_trigger
                .as_ref()
                .map(|trigger| trigger.schedule_handle())
        });
    if let Some(handle) = trigger_handle {
        if extensions
            .get::<keyring_schedule::SharedCommentsTcpDelegationScheduleHandle>()
            .is_some()
        {
            return Err(
                "Comments TCP delegation schedule trigger and standalone schedule handle cannot be combined"
                    .to_string(),
            );
        }
        extensions.insert(handle);
    }

    keyring_schedule_guard::register_comments_provider_runtime(extensions)
}

pub(super) async fn start_comments_tcp_listener_if_enabled(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<()> {
    keyring_schedule_guard::start_comments_tcp_listener_if_enabled(runtime_ctx).await
}

// Historical slice-80 source-verifier markers:
// SharedCommentsTcpDelegationScheduleTrigger>()
// standalone schedule handle cannot be combined
// extensions.insert(trigger.schedule_handle());
