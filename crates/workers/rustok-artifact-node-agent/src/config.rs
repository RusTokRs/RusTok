use std::{env, time::Duration};

use rustok_artifact_node_transport::ModuleArtifactNodeAgentIdentity;
use rustok_modules::MODULE_ARTIFACT_NODE_ASSIGNMENT_LEASE_SECONDS;
use rustok_storage::StorageConfig;
use tonic::transport::Endpoint;

const CONTROLLER_ENDPOINT_ENV: &str = "RUSTOK_ARTIFACT_NODE_AGENT_CONTROLLER_ENDPOINT";
const SANDBOX_ENDPOINT_ENV: &str = "RUSTOK_ARTIFACT_NODE_AGENT_SANDBOX_ENDPOINT";
const NODE_ID_ENV: &str = "RUSTOK_ARTIFACT_NODE_AGENT_NODE_ID";
const AGENT_ID_ENV: &str = "RUSTOK_ARTIFACT_NODE_AGENT_ID";
const STORAGE_CONFIG_ENV: &str = "RUSTOK_ARTIFACT_NODE_AGENT_STORAGE_CONFIG_JSON";
const POLL_INTERVAL_ENV: &str = "RUSTOK_ARTIFACT_NODE_AGENT_POLL_INTERVAL_MS";
const HEARTBEAT_INTERVAL_ENV: &str = "RUSTOK_ARTIFACT_NODE_AGENT_HEARTBEAT_INTERVAL_MS";

const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;
const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 30_000;
const MIN_POLL_INTERVAL_MS: u64 = 100;
const MAX_POLL_INTERVAL_MS: u64 = 60_000;
const MIN_HEARTBEAT_INTERVAL_MS: u64 = 1_000;

/// Deployment-owned configuration for one independently supervised node agent.
/// TLS material is loaded separately through the two shared mTLS client
/// prefixes so this type never stores private key bytes or certificate paths.
pub struct ArtifactNodeAgentConfig {
    pub controller_endpoint: Endpoint,
    pub sandbox_endpoint: Endpoint,
    pub identity: ModuleArtifactNodeAgentIdentity,
    pub storage: StorageConfig,
    pub poll_interval: Duration,
    pub heartbeat_interval: Duration,
}

impl ArtifactNodeAgentConfig {
    pub fn from_environment() -> Result<Self, String> {
        let controller_endpoint = parse_endpoint(CONTROLLER_ENDPOINT_ENV)?;
        let sandbox_endpoint = parse_endpoint(SANDBOX_ENDPOINT_ENV)?;
        let node_id = uuid::Uuid::parse_str(&required_nonempty_env(NODE_ID_ENV)?)
            .map_err(|_| format!("{NODE_ID_ENV} must be a UUID"))?;
        let identity =
            ModuleArtifactNodeAgentIdentity::new(node_id, required_nonempty_env(AGENT_ID_ENV)?)?;
        let storage = serde_json::from_str(&required_nonempty_env(STORAGE_CONFIG_ENV)?)
            .map_err(|_| format!("{STORAGE_CONFIG_ENV} is invalid"))?;
        let poll_interval = Duration::from_millis(parse_bounded_milliseconds(
            POLL_INTERVAL_ENV,
            DEFAULT_POLL_INTERVAL_MS,
            MIN_POLL_INTERVAL_MS,
            MAX_POLL_INTERVAL_MS,
        )?);
        let heartbeat_interval = Duration::from_millis(parse_bounded_milliseconds(
            HEARTBEAT_INTERVAL_ENV,
            DEFAULT_HEARTBEAT_INTERVAL_MS,
            MIN_HEARTBEAT_INTERVAL_MS,
            max_heartbeat_interval_ms()?,
        )?);
        Ok(Self {
            controller_endpoint,
            sandbox_endpoint,
            identity,
            storage,
            poll_interval,
            heartbeat_interval,
        })
    }
}

fn parse_endpoint(name: &str) -> Result<Endpoint, String> {
    let value = required_nonempty_env(name)?;
    parse_endpoint_value(name, value)
}

fn parse_endpoint_value(name: &str, value: String) -> Result<Endpoint, String> {
    let endpoint = Endpoint::from_shared(value).map_err(|_| format!("{name} is invalid"))?;
    if endpoint.uri().scheme_str() != Some("https") {
        return Err(format!("{name} must use https"));
    }
    Ok(endpoint)
}

fn required_nonempty_env(name: &str) -> Result<String, String> {
    let value = env::var(name).map_err(|_| format!("{name} must be configured"))?;
    if value.is_empty() || value.trim() != value {
        return Err(format!("{name} must be a non-empty trimmed value"));
    }
    Ok(value)
}

fn parse_bounded_milliseconds(
    name: &str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<u64, String> {
    let value = match env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|_| format!("{name} must be a millisecond count"))?,
        Err(env::VarError::NotPresent) => default,
        Err(env::VarError::NotUnicode(_)) => {
            return Err(format!("{name} must be valid UTF-8"));
        }
    };
    if !(minimum..=maximum).contains(&value) {
        return Err(format!(
            "{name} must be between {minimum} and {maximum} milliseconds"
        ));
    }
    Ok(value)
}

fn max_heartbeat_interval_ms() -> Result<u64, String> {
    let lease_seconds = u64::try_from(MODULE_ARTIFACT_NODE_ASSIGNMENT_LEASE_SECONDS)
        .map_err(|_| "artifact node assignment lease is invalid".to_string())?;
    lease_seconds
        .checked_mul(500)
        .ok_or_else(|| "artifact node assignment lease is invalid".to_string())
}

#[cfg(test)]
mod tests {
    use super::{max_heartbeat_interval_ms, parse_bounded_milliseconds, parse_endpoint_value};

    #[test]
    fn intervals_are_bounded_and_heartbeat_stays_below_the_owner_lease() {
        assert!(parse_bounded_milliseconds("TEST", 1_000, 100, 2_000).is_ok());
        assert!(parse_bounded_milliseconds("TEST", 1_000, 1_001, 2_000).is_err());
        assert!(max_heartbeat_interval_ms().expect("lease bound") >= 1_000);
    }

    #[test]
    fn controller_and_sandbox_endpoints_require_https() {
        assert!(parse_endpoint_value("TEST", "https://node.example.test".to_string()).is_ok());
        assert!(parse_endpoint_value("TEST", "http://node.example.test".to_string()).is_err());
    }
}
