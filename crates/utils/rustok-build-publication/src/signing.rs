use std::{fs::OpenOptions, io::Write, path::PathBuf, time::Duration};

use rustok_modules::OciArtifactReference;
use thiserror::Error;
use tokio::{process::Command, time::timeout};

use crate::credentials::{
    RegistryCredentialError, RegistryCredentialLease, remove_private_directory,
    remove_private_file, validate_fixed_program,
};

/// Fixed predicate classes emitted by the publication boundary. Arbitrary
/// predicate labels are intentionally not accepted from callers because the
/// verification worker requires exactly these two signed attestations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CosignAttestationPredicate {
    SlsaProvenance,
    CycloneDx,
}

impl CosignAttestationPredicate {
    fn cosign_type(self) -> &'static str {
        match self {
            Self::SlsaProvenance => "https://slsa.dev/provenance/v1",
            Self::CycloneDx => "https://cyclonedx.org/bom",
        }
    }
}

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

    /// Creates one signed in-toto attestation for the exact artifact subject.
    /// Cosign constructs the in-toto envelope and binds its subject to the
    /// digest-pinned OCI manifest. SLSA source evidence is therefore reduced
    /// to its validated predicate before it reaches the CLI; CycloneDX is
    /// already a predicate document. The resulting predicate bytes are staged
    /// in a private, create-new temporary file only because Cosign accepts the
    /// predicate through a file path.
    pub async fn attest(
        &self,
        artifact: &OciArtifactReference,
        credentials: &RegistryCredentialLease,
        predicate: CosignAttestationPredicate,
        predicate_bytes: &[u8],
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
        let normalized_predicate = normalize_cosign_predicate(predicate, predicate_bytes)?;
        let predicate_path = write_predicate_file(&normalized_predicate)?;
        let docker_config = match credentials.write_cosign_docker_config(&artifact.registry) {
            Ok(directory) => directory,
            Err(error) => {
                remove_private_file(&predicate_path);
                return Err(error.into());
            }
        };
        let result = async {
            let mut command = Command::new(&self.program);
            command
                .env_clear()
                .args([
                    "attest",
                    "--yes",
                    "--key",
                    &self.key_reference,
                    "--type",
                    predicate.cosign_type(),
                    "--predicate",
                ])
                .arg(&predicate_path)
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
        remove_private_file(&predicate_path);
        result
    }
}

/// Produces exactly the JSON predicate accepted by `cosign attest`.
///
/// The isolated build output and the canonical Alloy publication input carry a
/// complete SLSA statement so the source boundary can validate its original
/// payload subject before publication. Passing that complete statement to
/// `cosign attest --predicate` would nest it inside a new statement and lose
/// the expected SLSA shape. Cosign owns the final envelope and OCI-manifest
/// subject, so only the validated SLSA predicate is supplied to it.
fn normalize_cosign_predicate(
    predicate: CosignAttestationPredicate,
    evidence_bytes: &[u8],
) -> Result<Vec<u8>, CosignSigningError> {
    let document: serde_json::Value =
        serde_json::from_slice(evidence_bytes).map_err(|_| CosignSigningError::Rejected)?;
    match predicate {
        CosignAttestationPredicate::SlsaProvenance => {
            if document.get("_type").and_then(serde_json::Value::as_str)
                != Some("https://in-toto.io/Statement/v1")
                || document
                    .get("predicateType")
                    .and_then(serde_json::Value::as_str)
                    != Some(predicate.cosign_type())
                || document
                    .get("subject")
                    .and_then(serde_json::Value::as_array)
                    .is_none_or(Vec::is_empty)
            {
                return Err(CosignSigningError::Rejected);
            }
            let predicate = document
                .get("predicate")
                .filter(|value| value.is_object())
                .ok_or(CosignSigningError::Rejected)?;
            serde_json::to_vec(predicate).map_err(|_| CosignSigningError::Rejected)
        }
        CosignAttestationPredicate::CycloneDx => {
            if document
                .get("bomFormat")
                .and_then(serde_json::Value::as_str)
                != Some("CycloneDX")
                || document
                    .get("specVersion")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(str::is_empty)
            {
                return Err(CosignSigningError::Rejected);
            }
            serde_json::to_vec(&document).map_err(|_| CosignSigningError::Rejected)
        }
    }
}

fn write_predicate_file(predicate_bytes: &[u8]) -> Result<PathBuf, CosignSigningError> {
    let path = std::env::temp_dir().join(format!(
        "rustok-cosign-predicate-{}.json",
        uuid::Uuid::new_v4()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| CosignSigningError::Unavailable(error.to_string()))?;
        file.write_all(predicate_bytes)
            .and_then(|_| file.sync_all())
            .map_err(|error| CosignSigningError::Unavailable(error.to_string()))?;
        Ok(path.clone())
    })();
    if result.is_err() {
        remove_private_file(&path);
    }
    result
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
    use serde_json::json;

    use super::{CosignAttestationPredicate, normalize_cosign_predicate, write_predicate_file};
    use crate::credentials::remove_private_file;

    #[test]
    fn attestation_predicates_use_explicit_current_standard_uris() {
        assert_eq!(
            CosignAttestationPredicate::SlsaProvenance.cosign_type(),
            "https://slsa.dev/provenance/v1"
        );
        assert_eq!(
            CosignAttestationPredicate::CycloneDx.cosign_type(),
            "https://cyclonedx.org/bom"
        );
    }

    #[test]
    fn predicate_file_is_create_only_and_removed_by_the_private_cleanup_guard() {
        let path =
            write_predicate_file(br#"{"predicate":"value"}"#).expect("create predicate file");
        assert_eq!(
            std::fs::read(&path).expect("read predicate file"),
            br#"{"predicate":"value"}"#
        );
        remove_private_file(&path);
        assert!(!path.exists());
    }

    #[test]
    fn slsa_statement_is_reduced_to_its_predicate_before_cosign_attestation() {
        let source_statement = json!({
            "_type": "https://in-toto.io/Statement/v1",
            "predicateType": "https://slsa.dev/provenance/v1",
            "subject": [{ "digest": { "sha256": "a".repeat(64) } }],
            "predicate": {
                "buildDefinition": { "buildType": "https://rustok.dev/build/test" },
                "runDetails": { "builder": { "id": "https://rustok.dev/builder" } }
            }
        });

        let predicate = normalize_cosign_predicate(
            CosignAttestationPredicate::SlsaProvenance,
            &serde_json::to_vec(&source_statement).expect("statement JSON"),
        )
        .expect("normalized SLSA predicate");
        let predicate: serde_json::Value =
            serde_json::from_slice(&predicate).expect("predicate JSON");

        assert!(predicate.get("_type").is_none());
        assert_eq!(
            predicate["buildDefinition"]["buildType"],
            "https://rustok.dev/build/test"
        );
    }

    #[test]
    fn malformed_slsa_statement_or_cyclonedx_predicate_is_rejected_before_cosign() {
        assert!(
            normalize_cosign_predicate(
                CosignAttestationPredicate::SlsaProvenance,
                br#"{\"predicateType\":\"https://slsa.dev/provenance/v1\",\"predicate\":{}}"#,
            )
            .is_err()
        );
        assert!(
            normalize_cosign_predicate(
                CosignAttestationPredicate::CycloneDx,
                br#"{\"bomFormat\":\"CycloneDX\"}"#,
            )
            .is_err()
        );
    }
}
