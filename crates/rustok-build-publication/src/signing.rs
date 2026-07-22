use std::{path::PathBuf, time::Duration};

use rustok_modules::OciArtifactReference;
use thiserror::Error;
use tokio::{process::Command, time::timeout};

use crate::credentials::{
    remove_private_directory, validate_fixed_program, RegistryCredentialError,
    RegistryCredentialLease,
};

pub struct CosignArtifactSigner {
    program: PathBuf,
    program_digest: String,
    key_reference: String,
}

#[derive(Debug, Error)]
pub enum CosignSigningError {
    #[error("artifact signing was rejected")]
    Rejected,
    #[error("artifact signing timed out")]
    TimedOut,
    #[error("artifact signer is unavailable: {0}")]
    Unavailable(String),
    #[error(transparent)]
    Credential(#[from] RegistryCredentialError),
}

impl CosignArtifactSigner {
    pub fn new(
        program: PathBuf,
        program_digest: String,
        key_reference: String,
    ) -> Result<Self, String> {
        validate_fixed_program(&program, &program_digest, "Cosign program")?;
        if !is_kms_key_reference(&key_reference) {
            return Err("Cosign key reference must use an approved KMS provider URI".to_string());
        }
        Ok(Self {
            program,
            program_digest,
            key_reference,
        })
    }

    pub fn is_ready(&self) -> bool {
        validate_fixed_program(&self.program, &self.program_digest, "Cosign program").is_ok()
            && is_kms_key_reference(&self.key_reference)
    }

    pub async fn sign(
        &self,
        artifact: &OciArtifactReference,
        credentials: &RegistryCredentialLease,
        execution_timeout: Duration,
    ) -> Result<(), CosignSigningError> {
        if !self.is_ready() {
            return Err(CosignSigningError::Unavailable(
                "Cosign deployment identity changed".to_string(),
            ));
        }
        artifact
            .validate()
            .map_err(|_| CosignSigningError::Rejected)?;
        if execution_timeout.is_zero() {
            return Err(CosignSigningError::TimedOut);
        }
        let docker_config = credentials.write_cosign_docker_config(&artifact.registry)?;
        let result = async {
            let mut command = Command::new(&self.program);
            command
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
        remove_private_directory(&docker_config);
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
