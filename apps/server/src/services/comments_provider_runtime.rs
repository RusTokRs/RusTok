use std::{env, net::SocketAddr, sync::Arc};

use rustok_comments::{
    CommentsThreadPort, CommentsThreadTransport, TcpJsonCommentsTransport,
    remote_comments_thread_port,
};
use rustok_core::ModuleRuntimeExtensions;

pub const COMMENTS_PROVIDER_MODE_ENV: &str = "RUSTOK_COMMENTS_PROVIDER_MODE";
pub const COMMENTS_TCP_ENDPOINT_ENV: &str = "RUSTOK_COMMENTS_TCP_ENDPOINT";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsProviderProfile {
    InProcessFallback,
    Preconfigured,
    TcpLoopback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommentsProviderRuntimeSelection {
    pub profile: CommentsProviderProfile,
    pub endpoint: Option<SocketAddr>,
}

/// Publishes the host-selected Comments provider through `ModuleRuntimeExtensions`.
///
/// The default `in_process` mode intentionally inserts no port. Blog therefore
/// retains its existing database/event-bus fallback. `tcp` publishes the typed
/// remote adapter only for an explicit loopback sidecar endpoint; plaintext TCP
/// is never enabled for a non-loopback address.
pub fn register_comments_provider_runtime(
    extensions: &mut ModuleRuntimeExtensions,
) -> Result<(), String> {
    if extensions.contains::<Arc<dyn CommentsThreadPort>>() {
        extensions.insert(CommentsProviderRuntimeSelection {
            profile: CommentsProviderProfile::Preconfigured,
            endpoint: None,
        });
        return Ok(());
    }

    let mode = env::var(COMMENTS_PROVIDER_MODE_ENV)
        .unwrap_or_else(|_| "in_process".to_string())
        .trim()
        .to_ascii_lowercase();

    match mode.as_str() {
        "in_process" => {
            extensions.insert(CommentsProviderRuntimeSelection {
                profile: CommentsProviderProfile::InProcessFallback,
                endpoint: None,
            });
            Ok(())
        }
        "tcp" => {
            let raw_endpoint = env::var(COMMENTS_TCP_ENDPOINT_ENV).map_err(|_| {
                format!(
                    "{COMMENTS_TCP_ENDPOINT_ENV} is required when {COMMENTS_PROVIDER_MODE_ENV}=tcp"
                )
            })?;
            let endpoint = raw_endpoint.trim().parse::<SocketAddr>().map_err(|_| {
                format!(
                    "{COMMENTS_TCP_ENDPOINT_ENV} must be an explicit IP socket address"
                )
            })?;
            if !endpoint.ip().is_loopback() {
                return Err(format!(
                    "{COMMENTS_TCP_ENDPOINT_ENV} must be loopback while Comments TCP transport is unencrypted"
                ));
            }

            let transport: Arc<dyn CommentsThreadTransport> =
                Arc::new(TcpJsonCommentsTransport::new(endpoint));
            extensions.insert::<Arc<dyn CommentsThreadPort>>(remote_comments_thread_port(transport));
            extensions.insert(CommentsProviderRuntimeSelection {
                profile: CommentsProviderProfile::TcpLoopback,
                endpoint: Some(endpoint),
            });
            Ok(())
        }
        _ => Err(format!(
            "{COMMENTS_PROVIDER_MODE_ENV} must be one of: in_process, tcp"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_contract_exposes_profiles_and_environment_keys() {
        let selector: fn(&mut ModuleRuntimeExtensions) -> Result<(), String> =
            register_comments_provider_runtime;
        let _ = selector;
        assert_eq!(COMMENTS_PROVIDER_MODE_ENV, "RUSTOK_COMMENTS_PROVIDER_MODE");
        assert_eq!(COMMENTS_TCP_ENDPOINT_ENV, "RUSTOK_COMMENTS_TCP_ENDPOINT");
    }
}
