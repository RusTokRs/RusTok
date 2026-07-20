use std::{path::PathBuf, time::Duration};

use rustok_modules::OciArtifactReference;
use tokio::{process::Command, time::timeout};

use crate::{RegistryCredentialError, RegistryCredentialLease};

/// Fixed Cosign adapter used only after the worker has published a verified,
/// digest-pinned artifact. It accepts a KMS key reference, never key material,
/// and does not expose command output to build logs.
pub struct CosignArtifactSigner {
    program: PathBuf,
    key_reference: String,
}

#[derive(Debug)]
pub enum CosignSigningError {
    Rejected,
    TimedOut,
    Unavailable(String),
    Credential(RegistryCredentialError),
}

impl CosignArtifactSigner {
    pub fn from_env() -> Result<Self, String> {
        let program = PathBuf::from(
            std::env::var("RUSTOK_MODULE_BUILD_COSIGN_PROGRAM")
                .map_err(|_| "RUSTOK_MODULE_BUILD_COSIGN_PROGRAM must be configured".to_string())?,
        );
        let key_reference =
            std::env::var("RUSTOK_MODULE_BUILD_COSIGN_KEY_REFERENCE").map_err(|_| {
                "RUSTOK_MODULE_BUILD_COSIGN_KEY_REFERENCE must be configured".to_string()
            })?;
        Self::new(program, key_reference)
    }

    pub fn new(program: PathBuf, key_reference: String) -> Result<Self, String> {
        if !program.is_absolute() {
            return Err("module build Cosign program must be absolute".to_string());
        }
        let metadata = std::fs::symlink_metadata(&program).map_err(|error| {
            format!(
                "module build Cosign program {} cannot be inspected: {error}",
                program.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(
                "module build Cosign program must be a regular non-symlink file".to_string(),
            );
        }
        if !is_kms_key_reference(&key_reference) {
            return Err(
                "module build Cosign key reference must use an approved KMS provider URI"
                    .to_string(),
            );
        }
        Ok(Self {
            program,
            key_reference,
        })
    }

    pub async fn sign(
        &self,
        artifact: &OciArtifactReference,
        credentials: &RegistryCredentialLease,
        execution_timeout: Duration,
    ) -> Result<(), CosignSigningError> {
        artifact
            .validate()
            .map_err(|_| CosignSigningError::Rejected)?;
        if execution_timeout.is_zero() {
            return Err(CosignSigningError::TimedOut);
        }
        let docker_config = credentials
            .write_cosign_docker_config(&artifact.registry)
            .map_err(CosignSigningError::Credential)?;
        let result = async {
            let mut command = Command::new(&self.program);
            command
                // Do not inherit process-wide Cosign configuration, proxy
                // credentials, or alternate registry settings.
                .env_clear()
                .args(["sign", "--yes", "--key", &self.key_reference])
                .arg(artifact.canonical())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .env("DOCKER_CONFIG", &docker_config);
            let status = timeout(execution_timeout, command.status())
                .await
                .map_err(|_| CosignSigningError::TimedOut)?
                .map_err(|error| CosignSigningError::Unavailable(error.to_string()))?;
            if status.success() {
                Ok(())
            } else {
                Err(CosignSigningError::Rejected)
            }
        }
        .await;
        let _ = std::fs::remove_dir_all(docker_config);
        result
    }
}

fn is_kms_key_reference(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && !value.chars().any(char::is_whitespace)
        && ["awskms://", "azurekms://", "gcpkms://", "hashivault://"]
            .iter()
            .any(|prefix| value.starts_with(prefix) && value.len() > prefix.len())
}

#[cfg(test)]
mod tests {
    use super::is_kms_key_reference;

    #[test]
    fn accepts_only_non_file_kms_key_references() {
        assert!(is_kms_key_reference(
            "awskms:///arn:aws:kms:eu-west-1:123456789012:key/example"
        ));
        assert!(is_kms_key_reference(
            "gcpkms://projects/example/locations/global/keyRings/ring/cryptoKeys/key"
        ));
        assert!(!is_kms_key_reference("file:///secrets/cosign.key"));
        assert!(!is_kms_key_reference("awskms://"));
    }
}
