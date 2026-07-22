use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use oci_distribution::secrets::RegistryAuth;
use rustok_modules::OciArtifactPublicationTarget;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::timeout,
};
use uuid::Uuid;

const CREDENTIAL_REQUEST_CONTRACT: &str = "rustok.registry_credential.request";
const CREDENTIAL_RESPONSE_CONTRACT: &str = "rustok.registry_credential.response";
const MAX_CREDENTIAL_BROKER_OUTPUT_BYTES: usize = 16 * 1024;
const MAX_CREDENTIAL_LEASE_SECONDS: u64 = 15 * 60;

#[async_trait]
pub trait RegistryCredentialBroker: Send + Sync {
    async fn acquire(
        &self,
        target: &OciArtifactPublicationTarget,
        minimum_ttl: Duration,
    ) -> Result<RegistryCredentialLease, RegistryCredentialError>;

    fn is_ready(&self) -> bool;
}

pub struct CommandRegistryCredentialBroker {
    program: PathBuf,
    program_digest: String,
}

pub struct RegistryCredentialLease {
    username: String,
    password: String,
    expires_at_unix_seconds: u64,
}

#[derive(Debug, Error)]
pub enum RegistryCredentialError {
    #[error("registry credential request was rejected")]
    Rejected,
    #[error("registry credential request timed out")]
    TimedOut,
    #[error("registry credential broker is unavailable: {0}")]
    Unavailable(String),
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct RegistryCredentialRequest<'a> {
    contract: &'static str,
    registry: &'a str,
    repository: &'a str,
    minimum_ttl_seconds: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryCredentialResponse {
    contract: String,
    registry: String,
    repository: String,
    username: String,
    password: String,
    expires_at_unix_seconds: u64,
}

impl CommandRegistryCredentialBroker {
    pub fn new(program: PathBuf, program_digest: String) -> Result<Self, String> {
        validate_fixed_program(&program, &program_digest, "registry credential broker")?;
        Ok(Self {
            program,
            program_digest,
        })
    }

    fn validate_runtime(&self) -> Result<(), RegistryCredentialError> {
        validate_fixed_program(
            &self.program,
            &self.program_digest,
            "registry credential broker",
        )
        .map_err(RegistryCredentialError::Unavailable)
    }
}

#[async_trait]
impl RegistryCredentialBroker for CommandRegistryCredentialBroker {
    async fn acquire(
        &self,
        target: &OciArtifactPublicationTarget,
        minimum_ttl: Duration,
    ) -> Result<RegistryCredentialLease, RegistryCredentialError> {
        self.validate_runtime()?;
        target
            .validate()
            .map_err(|_| RegistryCredentialError::Rejected)?;
        let minimum_ttl_seconds = minimum_ttl
            .as_secs()
            .max(1)
            .min(MAX_CREDENTIAL_LEASE_SECONDS);
        let request = serde_json::to_vec(&RegistryCredentialRequest {
            contract: CREDENTIAL_REQUEST_CONTRACT,
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

    fn is_ready(&self) -> bool {
        self.validate_runtime().is_ok()
    }
}

impl RegistryCredentialLease {
    fn from_response(
        response: RegistryCredentialResponse,
        target: &OciArtifactPublicationTarget,
        minimum_ttl_seconds: u64,
    ) -> Result<Self, RegistryCredentialError> {
        let now = current_unix_seconds()?;
        if response.contract != CREDENTIAL_RESPONSE_CONTRACT
            || response.registry != target.registry
            || response.repository != target.repository
            || response.username.trim().is_empty()
            || response.username.trim() != response.username
            || response.username.chars().any(char::is_control)
            || response.password.is_empty()
            || response.password.len() > MAX_CREDENTIAL_BROKER_OUTPUT_BYTES
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

    pub fn ensure_valid(&self) -> Result<(), RegistryCredentialError> {
        if self.expires_at_unix_seconds <= current_unix_seconds()? {
            return Err(RegistryCredentialError::Rejected);
        }
        Ok(())
    }

    pub fn write_cosign_docker_config(
        &self,
        registry: &str,
    ) -> Result<PathBuf, RegistryCredentialError> {
        self.ensure_valid()?;
        if registry.trim().is_empty()
            || registry.trim() != registry
            || registry.chars().any(char::is_control)
        {
            return Err(RegistryCredentialError::Rejected);
        }
        let directory = std::env::temp_dir().join(format!("rustok-cosign-auth-{}", Uuid::new_v4()));
        std::fs::create_dir(&directory)
            .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))?;
        let result = (|| {
            let auth = STANDARD.encode(format!("{}:{}", self.username, self.password));
            let mut auths = serde_json::Map::new();
            auths.insert(registry.to_string(), serde_json::json!({ "auth": auth }));
            let bytes = serde_json::to_vec(&serde_json::json!({ "auths": auths }))
                .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))?;
            let config_path = directory.join("config.json");
            let mut config = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&config_path)
                .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))?;
            config
                .write_all(&bytes)
                .and_then(|_| config.sync_all())
                .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))?;
            Ok(directory.clone())
        })();
        if result.is_err() {
            remove_private_directory(&directory);
        }
        result
    }
}

pub(crate) fn validate_fixed_program(
    path: &Path,
    expected_digest: &str,
    label: &str,
) -> Result<(), String> {
    if !path.is_absolute() || !valid_digest(expected_digest) {
        return Err(format!("{label} path or digest is invalid"));
    }
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("{label} cannot be inspected: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{label} must be a non-symlink regular file"));
    }
    let mut file = File::open(path).map_err(|error| format!("{label} cannot be read: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("{label} cannot be read: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("sha256:{}", hex::encode(hasher.finalize()));
    if actual != expected_digest {
        return Err(format!("{label} digest does not match"));
    }
    Ok(())
}

pub(crate) fn remove_private_directory(path: &Path) {
    if path.is_absolute()
        && path.parent() == Some(std::env::temp_dir().as_path())
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("rustok-cosign-auth-"))
        && std::fs::symlink_metadata(path)
            .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        let _ = std::fs::remove_dir_all(path);
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

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn current_unix_seconds() -> Result<u64, RegistryCredentialError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| RegistryCredentialError::Unavailable(error.to_string()))
}
