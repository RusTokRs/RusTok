use std::{env, sync::Arc};

use rustok_core::ModuleRuntimeExtensions;

use crate::error::{Error, Result};
use crate::services::server_runtime_context::ServerRuntimeContext;

use super::{base, keyring_reload, keyring_schedule};

pub(super) fn register_comments_provider_runtime(
    extensions: &mut ModuleRuntimeExtensions,
) -> std::result::Result<(), String> {
    validate_optional_bool(keyring_schedule::COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED_ENV)?;
    validate_optional_bool(keyring_reload::COMMENTS_TCP_DELEGATION_RELOAD_ENABLED_ENV)?;
    validate_schedule_runtime_policy(
        extensions.get::<keyring_schedule::SharedCommentsTcpDelegationScheduleHandle>(),
    )?;
    keyring_schedule::register_comments_provider_runtime(extensions)
}

pub(super) async fn start_comments_tcp_listener_if_enabled(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<()> {
    let extensions = runtime_ctx.shared_get::<Arc<ModuleRuntimeExtensions>>();
    let handle = runtime_ctx
        .shared_get::<keyring_schedule::SharedCommentsTcpDelegationScheduleHandle>()
        .or_else(|| {
            extensions.as_ref().and_then(|values| {
                values
                    .get::<keyring_schedule::SharedCommentsTcpDelegationScheduleHandle>()
                    .cloned()
            })
        });
    validate_schedule_runtime_policy(handle.as_ref()).map_err(Error::BadRequest)?;
    keyring_schedule::start_comments_tcp_listener_if_enabled(runtime_ctx).await
}

fn validate_schedule_runtime_policy(
    handle: Option<&keyring_schedule::SharedCommentsTcpDelegationScheduleHandle>,
) -> std::result::Result<(), String> {
    let Some(handle) = handle else {
        return Ok(());
    };
    let selection = handle.current_selection()?;
    let runtime_ttl_ms = read_runtime_ttl_ms()?;
    if selection.max_ttl_ms != runtime_ttl_ms
        || selection.clock_skew_ms != rustok_comments::DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS
    {
        return Err(
            "Comments TCP delegation schedule TTL and clock-skew policy must match the built-in signer and resolver runtime policy"
                .to_string(),
        );
    }
    Ok(())
}

fn read_runtime_ttl_ms() -> std::result::Result<u64, String> {
    let ttl_ms = match env::var(base::COMMENTS_TCP_DELEGATION_TTL_MS_ENV) {
        Ok(value) => parse_positive_u64(base::COMMENTS_TCP_DELEGATION_TTL_MS_ENV, &value)?,
        Err(env::VarError::NotPresent) => rustok_comments::DEFAULT_COMMENTS_TCP_DELEGATION_TTL_MS,
        Err(env::VarError::NotUnicode(_)) => {
            return Err(format!(
                "{} must contain valid UTF-8",
                base::COMMENTS_TCP_DELEGATION_TTL_MS_ENV
            ));
        }
    };
    if ttl_ms > rustok_comments::MAX_COMMENTS_TCP_DELEGATION_TTL_MS {
        return Err(format!(
            "{} must be within 1..={}",
            base::COMMENTS_TCP_DELEGATION_TTL_MS_ENV,
            rustok_comments::MAX_COMMENTS_TCP_DELEGATION_TTL_MS
        ));
    }
    Ok(ttl_ms)
}

fn validate_optional_bool(key: &'static str) -> std::result::Result<(), String> {
    let value = match env::var(key) {
        Ok(value) => value,
        Err(env::VarError::NotPresent) => return Ok(()),
        Err(env::VarError::NotUnicode(_)) => {
            return Err(format!("{key} must contain valid UTF-8"));
        }
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "0" | "false" | "no" | "off" => Ok(()),
        _ => Err(format!(
            "{key} must be one of: true, false, 1, 0, yes, no, on, off"
        )),
    }
}

fn parse_positive_u64(key: &'static str, value: &str) -> std::result::Result<u64, String> {
    let parsed = value
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("{key} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("{key} must be greater than zero"));
    }
    Ok(parsed)
}
