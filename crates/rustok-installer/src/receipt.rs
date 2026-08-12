use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{plan::InstallDistributionBinding, state::InstallStep};

#[cfg(feature = "host-runtime")]
const MAX_BASE_DISTRIBUTION_RECEIPT_BYTES: u64 = 256 * 1024;

#[derive(Debug, Error)]
pub enum ReceiptError {
    #[error("failed to serialize receipt input for checksum: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("base-distribution receipt is invalid")]
    InvalidBaseDistributionReceipt,
    #[error("base-distribution receipt verification failed: {0}")]
    BaseDistributionVerification(
        #[from] rustok_modules::ModuleStaticDistributionBootstrapReceiptError,
    ),
    #[error("failed to read base-distribution receipt `{path}`: {source}")]
    BaseDistributionReceiptIo {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug)]
pub struct VerifiedInstallBaseDistributionReceipt {
    receipt: rustok_modules::VerifiedModuleStaticDistributionBootstrapReceipt,
}

impl VerifiedInstallBaseDistributionReceipt {
    pub fn payload(&self) -> &rustok_modules::ModuleStaticDistributionBootstrapReceiptPayload {
        self.receipt.payload()
    }

    pub fn into_binding(self) -> Result<InstallDistributionBinding, ReceiptError> {
        let payload = self.receipt.payload();
        let binding = InstallDistributionBinding {
            preparation_id: payload.preparation_id,
            distribution_release_id: payload.distribution_release_id,
            bundle_reference: payload.preparation.evidence.bundle_reference.clone(),
            bundle_root_digest: payload.preparation.evidence.bundle_root_digest.clone(),
            role_set_digest: payload.preparation.evidence.role_set_digest.clone(),
            roles: payload.preparation.evidence.roles.clone(),
            bootstrap_receipt: Some(Box::new(self.receipt.into_receipt())),
        };
        binding
            .validate()
            .map_err(|_| ReceiptError::InvalidBaseDistributionReceipt)?;
        Ok(binding)
    }
}

#[cfg(feature = "host-runtime")]
pub fn load_base_distribution_receipt(
    path: impl AsRef<std::path::Path>,
    public_key_base64: &str,
    now: DateTime<Utc>,
) -> Result<VerifiedInstallBaseDistributionReceipt, ReceiptError> {
    use std::io::Read;

    let path = path.as_ref();
    let metadata = std::fs::symlink_metadata(path).map_err(|source| {
        ReceiptError::BaseDistributionReceiptIo {
            path: path.display().to_string(),
            source,
        }
    })?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_BASE_DISTRIBUTION_RECEIPT_BYTES {
        return Err(ReceiptError::InvalidBaseDistributionReceipt);
    }
    let file =
        std::fs::File::open(path).map_err(|source| ReceiptError::BaseDistributionReceiptIo {
            path: path.display().to_string(),
            source,
        })?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_BASE_DISTRIBUTION_RECEIPT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| ReceiptError::BaseDistributionReceiptIo {
            path: path.display().to_string(),
            source,
        })?;
    if bytes.len() as u64 > MAX_BASE_DISTRIBUTION_RECEIPT_BYTES {
        return Err(ReceiptError::InvalidBaseDistributionReceipt);
    }
    let receipt: rustok_modules::ModuleStaticDistributionBootstrapReceipt =
        serde_json::from_slice(&bytes)?;
    let receipt = receipt
        .verify_owned(public_key_base64, now)
        .map_err(ReceiptError::from)?;
    Ok(VerifiedInstallBaseDistributionReceipt { receipt })
}

#[cfg(test)]
fn sha256_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptOutcome {
    Success,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallReceipt {
    pub session_id: String,
    pub step: InstallStep,
    pub input_checksum: String,
    pub outcome: ReceiptOutcome,
    pub diagnostics: serde_json::Value,
    pub installer_version: String,
    pub created_at: DateTime<Utc>,
}

impl InstallReceipt {
    pub fn success<T: Serialize>(
        session_id: impl Into<String>,
        step: InstallStep,
        input: &T,
        diagnostics: serde_json::Value,
    ) -> Result<Self, ReceiptError> {
        Ok(Self {
            session_id: session_id.into(),
            step,
            input_checksum: checksum_json(input)?,
            outcome: ReceiptOutcome::Success,
            diagnostics,
            installer_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: Utc::now(),
        })
    }

    pub fn can_skip<T: Serialize>(
        &self,
        step: InstallStep,
        input: &T,
    ) -> Result<bool, ReceiptError> {
        Ok(self.step == step
            && self.outcome == ReceiptOutcome::Success
            && self.input_checksum == checksum_json(input)?)
    }
}

pub fn checksum_json<T: Serialize>(value: &T) -> Result<String, ReceiptError> {
    let bytes = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use chrono::Duration;
    use ed25519_dalek::{Signer, SigningKey};
    use uuid::Uuid;

    use super::*;

    #[derive(Serialize)]
    struct Input {
        value: &'static str,
    }

    #[test]
    fn checksum_is_stable_for_same_json() {
        let left = checksum_json(&Input { value: "same" }).unwrap();
        let right = checksum_json(&Input { value: "same" }).unwrap();

        assert_eq!(left, right);
    }

    #[test]
    fn receipt_can_skip_matching_successful_step() {
        let input = Input { value: "db-ready" };
        let receipt = InstallReceipt::success(
            "is_01",
            InstallStep::Database,
            &input,
            serde_json::json!({}),
        )
        .unwrap();

        assert!(receipt.can_skip(InstallStep::Database, &input).unwrap());
        assert!(!receipt.can_skip(InstallStep::Migrate, &input).unwrap());
    }

    #[test]
    fn signed_base_distribution_receipt_creates_exact_binding() {
        let now = DateTime::parse_from_rfc3339("2026-08-12T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let (receipt, public_key) = signed_base_distribution_receipt(now);

        let verified = VerifiedInstallBaseDistributionReceipt {
            receipt: receipt.verify_owned(&public_key, now).unwrap(),
        };
        let binding = verified.into_binding().unwrap();

        assert_eq!(binding.preparation_id, Uuid::from_u128(1));
        assert_eq!(binding.distribution_release_id, Uuid::from_u128(2));
        assert_eq!(
            binding.bundle_reference,
            format!("registry.example/rustok/base@{}", digest('b'))
        );
        assert_eq!(binding.bundle_root_digest, digest('b'));
        assert_eq!(
            binding.role_set_digest,
            rustok_modules::ModuleStaticDistributionBuildEvidence::role_set_digest(&binding.roles)
                .unwrap()
        );
        assert!(binding.bootstrap_receipt.is_some());
    }

    #[test]
    fn signed_base_distribution_receipt_rejects_tampering_and_expiry() {
        let now = DateTime::parse_from_rfc3339("2026-08-12T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let (mut tampered, public_key) = signed_base_distribution_receipt(now);
        tampered.payload.preparation.evidence.bundle_root_digest = digest('d');
        assert!(matches!(
            tampered.verify_owned(&public_key, now),
            Err(rustok_modules::ModuleStaticDistributionBootstrapReceiptError::SignatureRejected)
        ));

        let (expired, public_key) = signed_base_distribution_receipt(now);
        assert!(matches!(
            expired.verify_owned(&public_key, now + Duration::hours(2)),
            Err(rustok_modules::ModuleStaticDistributionBootstrapReceiptError::Expired)
        ));
    }

    #[test]
    fn distribution_binding_rejects_receipt_identity_mismatch() {
        let now = DateTime::parse_from_rfc3339("2026-08-12T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let (receipt, public_key) = signed_base_distribution_receipt(now);
        let mut binding = VerifiedInstallBaseDistributionReceipt {
            receipt: receipt.verify_owned(&public_key, now).unwrap(),
        }
        .into_binding()
        .unwrap();
        binding.role_set_digest = digest('d');

        assert_eq!(
            binding.validate().unwrap_err(),
            "distribution binding does not match its signed bootstrap receipt"
        );
    }

    #[cfg(feature = "host-runtime")]
    #[test]
    fn base_distribution_loader_verifies_the_bounded_regular_file() {
        let now = DateTime::parse_from_rfc3339("2026-08-12T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let (receipt, public_key) = signed_base_distribution_receipt(now);
        let path = std::env::temp_dir().join(format!(
            "rustok-base-distribution-receipt-{}.json",
            Uuid::new_v4()
        ));
        std::fs::write(&path, serde_json::to_vec(&receipt).unwrap()).unwrap();

        let verified = load_base_distribution_receipt(&path, &public_key, now).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(
            verified.payload().distribution_release_id,
            receipt.payload.distribution_release_id
        );
    }

    fn signed_base_distribution_receipt(
        now: DateTime<Utc>,
    ) -> (
        rustok_modules::ModuleStaticDistributionBootstrapReceipt,
        String,
    ) {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let public_key = signing_key.verifying_key().to_bytes();
        let platform_source_digest = digest('1');
        let items = Vec::new();
        let roles = vec![rustok_modules::ModuleStaticDistributionRoleArtifact {
            role: rustok_modules::ModuleStaticDistributionRole::Monolith,
            artifact_digest: digest('2'),
        }];
        let role_set_digest =
            rustok_modules::ModuleStaticDistributionBuildEvidence::role_set_digest(&roles).unwrap();
        let preparation = rustok_modules::ModuleStaticDistributionBootstrapPreparation {
            composition_revision: 1,
            composition_digest: rustok_modules::module_static_distribution_composition_digest(
                &format!("cas://{platform_source_digest}"),
                &platform_source_digest,
                &digest('3'),
                "x86_64-unknown-linux-gnu",
                &items,
            )
            .unwrap(),
            platform_source_reference: format!("cas://{platform_source_digest}"),
            platform_source_digest,
            toolchain_digest: digest('3'),
            build_target: "x86_64-unknown-linux-gnu".to_string(),
            items,
            evidence: rustok_modules::ModuleStaticDistributionBuildEvidence {
                bundle_reference: format!("registry.example/rustok/base@{}", digest('b')),
                bundle_root_digest: digest('b'),
                role_set_digest,
                roles,
                sbom_reference: "oci://base/sbom".to_string(),
                sbom_digest: digest('4'),
                provenance_reference: "oci://base/provenance".to_string(),
                provenance_digest: digest('5'),
                signature_reference: "oci://base/signature".to_string(),
                signature_digest: digest('6'),
                test_evidence_reference: "oci://base/tests".to_string(),
                test_evidence_digest: digest('7'),
            },
            admission: rustok_modules::ModuleStaticDistributionReleaseAdmission {
                verifier_identity: "platform-bootstrap-signer".to_string(),
                policy_revision: "policy@1".to_string(),
                evidence_reference: "oci://base/admission".to_string(),
                evidence_digest: digest('8'),
                signature_verified: true,
                provenance_verified: true,
                sbom_verified: true,
                test_evidence_verified: true,
                dependency_policy_verified: true,
            },
        };
        let payload = rustok_modules::ModuleStaticDistributionBootstrapReceiptPayload {
            contract: rustok_modules::MODULE_STATIC_DISTRIBUTION_BOOTSTRAP_RECEIPT_CONTRACT
                .to_string(),
            preparation_id: Uuid::from_u128(1),
            distribution_release_id: Uuid::from_u128(2),
            host_composition_revision: "distribution@1".to_string(),
            host_composition_hash: "a".repeat(64),
            preparation,
            migration_plan_digest: digest('d'),
            data_contract_digest: digest('e'),
            signer_key_digest: sha256_digest(&public_key),
            issued_at: now - Duration::minutes(1),
            expires_at: now + Duration::hours(1),
        };
        let signature =
            signing_key.sign(&rustok_api::manifest_hash::canonical_json_bytes(&payload).unwrap());
        (
            rustok_modules::ModuleStaticDistributionBootstrapReceipt {
                payload,
                signature: STANDARD.encode(signature.to_bytes()),
            },
            STANDARD.encode(public_key),
        )
    }

    fn digest(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }
}
