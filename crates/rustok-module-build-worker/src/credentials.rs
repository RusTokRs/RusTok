use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use oci_distribution::secrets::RegistryAuth;
use rustok_modules::OciArtifactPublicationTarget;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::timeout,
};
use uuid::Uuid;

const REGISTRY_CREDENTIAL_PROTOCOL_VERSION: u32 = 1;
const MAX_CREDENTIAL_BROKER_OUTPUT_BYTES: usize = 16 * 1024;
const MAX_CREDENTIAL_LEASE_SECONDS: u64 = 15 * 60;

/// Deployment-owned source of one short-lived registry credential lease. The
/// request identifies only the configured repository and never carries secret
/// material.
#[async_trait]
pub trait RegistryCredentialBroker: Send + Sync {
    async fn acquire(
        &self,
        target: &OciArtifactPublicationTarget,
        minimum_ttl: Duration,
    ) -> Result<RegistryCredentialLease, RegistryCredentialError>;
}

/// Fixed command adapter for a deployment-owned credential broker. The broker
/// may use its own workload identity or local channel but must never emit a
/// credential outside the bounded private stdout protocol.
pub struct CommandRegistryCredentialBroker {
    program: PathBuf,
}

/// Secret-bearing lease intentionally does not implement Debug, Serialize, or
/// Deserialize. Its value is usable only for registry authentication or a
/// short-lived private Cosign Docker configuration.
pub struct RegistryCredentialLease {
    username: String,
    password: String,
    expires_at_unix_seconds: u64,
}

#[derive(Debug)]
pub enum RegistryCredentialError {
    Rejected,
    TimedOut,
    Unavailable(String),
}

#[derive(Serialize)]
struct RegistryCredentialRequest<'a> {
    protocol_version: u32,
    registry: &'a str,
    repository: &'a str,
    minimum_ttl_seconds: u64,
}

#[derive(Deserialize)]
struct RegistryCredentialResponse {
    protocol_version: u32,
    registry: String,
    repository: String,
    username: String,
    password: String,
    expires_at_unix_seconds: u64,
}

impl CommandRegistryCredentialBroker {
    pub fn from_env() -> Result<Self, String> {
        let program = PathBuf::from(
            std::env::var("RUSTOK_MODULE_BUILD_REGISTRY_CREDENTIAL_BROKER").map_err(|_| {
                "RUSTOK_MODULE_BUILD_REGISTRY_CREDENTIAL_BROKER must be configured".to_string()
            })?,
        );
        Self::new(program)
    }

    pub fn new(program: PathBuf) -> Result<Self, String> {
        if !program.is_absolute() {
            return Err("module build registry credential broker must be absolute".to_string());
        }
        let metadata = std::fs::symlink_metadata(&program).map_err(|error| {
            format!(
                "module build registry credential broker {} cannot be inspected: {error}",
                program.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(
                "module build registry credential broker must be a regular non-symlink file"
                    .to_string(),
            );
        }
        Ok(Self { program })
    }
}

#[async_trait]
impl RegistryCredentialBroker for CommandRegistryCredentialBroker {
    async fn acquire(
        &self,
        target: &OciArtifactPublicationTarget,
        minimum_ttl: Duration,
    ) -> Result<RegistryCredentialLease, RegistryCredentialError> {
        target
            .validate()
            .map_err(|_| RegistryCredentialError::Rejected)?;
        let minimum_ttl_seconds = minimum_ttl
            .as_secs()
            .max(1)
            .min(MAX_CREDENTIAL_LEASE_SECONDS);
        let request = serde_json::to_vec(&RegistryCredentialRequest {
            protocol_version: REGISTRY_CREDENTIAL_PROTOCOL_VERSION,
            registry: &target.registry,
            repository: &target.repository,
            minimum_ttl_seconds,
        })
        .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))?;
        let mut child = Command::new(&self.program)
            .env_clear()
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))?;
        let mut stdin = child.stdin.take().ok_or_else(|| {
            RegistryCredentialError::Unavailable(
                "credential broker stdin is unavailable".to_string(),
            )
        })?;
        stdin
            .write_all(&request)
            .await
            .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))?;
        drop(stdin);
        let stdout = child.stdout.take().ok_or_else(|| {
            RegistryCredentialError::Unavailable(
                "credential broker stdout is unavailable".to_string(),
            )
        })?;
        let output = tokio::spawn(read_bounded(stdout));
        let status = timeout(minimum_ttl, child.wait())
            .await
            .map_err(|_| RegistryCredentialError::TimedOut)?
            .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))?;
        let output = output
            .await
            .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))?
            .map_err(|_| RegistryCredentialError::Rejected)?;
        if !status.success() {
            return Err(RegistryCredentialError::Rejected);
        }
        let response: RegistryCredentialResponse =
            serde_json::from_slice(&output).map_err(|_| RegistryCredentialError::Rejected)?;
        RegistryCredentialLease::from_response(response, target, minimum_ttl_seconds)
    }
}

impl RegistryCredentialLease {
    fn from_response(
        response: RegistryCredentialResponse,
        target: &OciArtifactPublicationTarget,
        minimum_ttl_seconds: u64,
    ) -> Result<Self, RegistryCredentialError> {
        let now = current_unix_seconds()?;
        if response.protocol_version != REGISTRY_CREDENTIAL_PROTOCOL_VERSION
            || response.registry != target.registry
            || response.repository != target.repository
            || response.username.trim().is_empty()
            || response.password.is_empty()
            || response.expires_at_unix_seconds < now.saturating_add(minimum_ttl_seconds)
            || response.expires_at_unix_seconds > now.saturating_add(MAX_CREDENTIAL_LEASE_SECONDS)
        {
            return Err(RegistryCredentialError::Rejected);
        }
        Ok(Self {
            username: response.username,
            password: response.password,
            expires_at_unix_seconds: response.expires_at_unix_seconds,
        })
    }

    pub fn registry_auth(&self) -> RegistryAuth {
        RegistryAuth::Basic(self.username.clone(), self.password.clone())
    }

    pub(crate) fn ensure_valid(&self) -> Result<(), RegistryCredentialError> {
        if self.expires_at_unix_seconds <= current_unix_seconds()? {
            return Err(RegistryCredentialError::Rejected);
        }
        Ok(())
    }

    pub(crate) fn write_cosign_docker_config(
        &self,
        registry: &str,
    ) -> Result<PathBuf, RegistryCredentialError> {
        self.ensure_valid()?;
        let directory = std::env::temp_dir().join(format!(
            "rustok-module-build-cosign-auth-{}",
            Uuid::new_v4()
        ));
        std::fs::create_dir(&directory)
            .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))?;
        let result = (|| {
            let auth = STANDARD.encode(format!("{}:{}", self.username, self.password));
            let mut auths = serde_json::Map::new();
            auths.insert(registry.to_string(), serde_json::json!({ "auth": auth }));
            let config = serde_json::json!({ "auths": auths });
            std::fs::write(
                directory.join("config.json"),
                serde_json::to_vec(&config)
                    .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))?,
            )
            .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))?;
            Ok(directory.clone())
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&directory);
        }
        result
    }
}

async fn read_bounded<R>(mut reader: R) -> Result<Vec<u8>, ()>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0_u8; 4 * 1024];
    loop {
        let read = reader.read(&mut buffer).await.map_err(|_| ())?;
        if read == 0 {
            return Ok(output);
        }
        if output.len().saturating_add(read) > MAX_CREDENTIAL_BROKER_OUTPUT_BYTES {
            return Err(());
        }
        output.extend_from_slice(&buffer[..read]);
    }
}

fn current_unix_seconds() -> Result<u64, RegistryCredentialError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{
        RegistryCredentialError, RegistryCredentialLease, RegistryCredentialResponse,
        REGISTRY_CREDENTIAL_PROTOCOL_VERSION,
    };
    use rustok_modules::OciArtifactPublicationTarget;

    #[test]
    fn rejects_an_expired_or_cross_repository_lease() {
        let target = OciArtifactPublicationTarget {
            registry: "registry.example".to_string(),
            repository: "modules/sample_module".to_string(),
        };
        let response = RegistryCredentialResponse {
            protocol_version: REGISTRY_CREDENTIAL_PROTOCOL_VERSION,
            registry: target.registry.clone(),
            repository: "modules/other_module".to_string(),
            username: "publisher".to_string(),
            password: "secret".to_string(),
            expires_at_unix_seconds: u64::MAX,
        };
        assert!(matches!(
            RegistryCredentialLease::from_response(response, &target, 1),
            Err(RegistryCredentialError::Rejected)
        ));

        let expired = RegistryCredentialResponse {
            protocol_version: REGISTRY_CREDENTIAL_PROTOCOL_VERSION,
            registry: target.registry.clone(),
            repository: target.repository.clone(),
            username: "publisher".to_string(),
            password: "secret".to_string(),
            expires_at_unix_seconds: 0,
        };
        assert!(matches!(
            RegistryCredentialLease::from_response(expired, &target, 1),
            Err(RegistryCredentialError::Rejected)
        ));
    }
}
