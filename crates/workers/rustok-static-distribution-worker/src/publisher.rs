use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::Duration,
};

use rustok_build_publication::{
    CommandRegistryCredentialBroker, CosignArtifactSigner, RegistryCredentialBroker,
};
use rustok_distribution::GeneratedStaticDistributionManifest;
use rustok_modules::{
    ModuleStaticDistributionBuildEvidence, ModuleStaticDistributionRole,
    ModuleStaticDistributionRoleArtifact, OciArtifactPublicationTarget,
    OciBuildPublicationArtifact, OciBuildPublicationBlob, OciDistributionArtifactPublisher,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::time::timeout;

use crate::{
    StaticDistributionPublicationReceipt, StaticDistributionPublisherRequest,
    StaticDistributionTestEvidence,
};

mod evidence;
use evidence::{build_cyclonedx_sbom, build_slsa_provenance};

const PUBLISHER_CONFIG_CONTRACT: &str = "rustok.static_distribution.publisher_config";
const ARTIFACT_CONFIG_CONTRACT: &str = "rustok.static_distribution.artifact";
const PUBLISHER_REQUEST_CONTRACT: &str = "rustok.static_distribution.publisher_request";
const PUBLICATION_RECEIPT_CONTRACT: &str = "rustok.static_distribution.publication_receipt";
const TEST_EVIDENCE_CONTRACT: &str = "rustok.static_distribution.test_evidence";
const ARTIFACT_CONFIG_MEDIA_TYPE: &str = "application/vnd.rustok.distribution.config+json";
const ARTIFACT_LAYER_MEDIA_TYPE: &str = "application/vnd.rustok.distribution.executable";
const SBOM_MEDIA_TYPE: &str = "application/vnd.cyclonedx+json";
const PROVENANCE_MEDIA_TYPE: &str = "application/vnd.in-toto+json";
const TEST_EVIDENCE_MEDIA_TYPE: &str = "application/vnd.rustok.distribution.test-evidence+json";
const GENERATED_MANIFEST_PATH: &str = ".rustok/generated/static-distribution.json";
const WORKSPACE_LOCK_PATH: &str = "Cargo.lock";
const MAX_CONFIG_BYTES: u64 = 64 * 1024;
const MAX_REQUEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TEST_EVIDENCE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_LOCK_BYTES: u64 = 32 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_EVIDENCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PUBLICATION_TIMEOUT_SECONDS: u64 = 14 * 60;
const CREDENTIAL_SAFETY_MARGIN: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticDistributionPublisherPaths {
    pub request: PathBuf,
    pub workspace: PathBuf,
    pub test_evidence: PathBuf,
    pub config: PathBuf,
    pub config_digest: String,
    pub receipt: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticDistributionPublisherConfig {
    pub contract: String,
    pub registry: String,
    pub repository: String,
    pub artifact_file_name: String,
    pub credential_broker_path: PathBuf,
    pub credential_broker_digest: String,
    pub cosign_path: PathBuf,
    pub cosign_digest: String,
    pub cosign_key_reference: String,
    pub max_artifact_bytes: u64,
    pub max_evidence_bytes: u64,
    pub publication_timeout_seconds: u64,
}

impl StaticDistributionPublisherConfig {
    pub fn load(
        path: &Path,
        expected_digest: &str,
    ) -> Result<Self, StaticDistributionPublisherError> {
        if !valid_digest(expected_digest) {
            return Err(StaticDistributionPublisherError::InvalidConfig(
                "publisher config digest is invalid".to_string(),
            ));
        }
        let bytes = read_bounded_regular(path, MAX_CONFIG_BYTES)?;
        if digest_bytes(&bytes) != expected_digest {
            return Err(StaticDistributionPublisherError::InvalidConfig(
                "publisher config digest does not match".to_string(),
            ));
        }
        let config: Self = serde_json::from_slice(&bytes).map_err(|error| {
            StaticDistributionPublisherError::InvalidConfig(format!(
                "publisher config JSON is invalid: {error}"
            ))
        })?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), StaticDistributionPublisherError> {
        let target = self.publication_target();
        if self.contract != PUBLISHER_CONFIG_CONTRACT
            || target.validate().is_err()
            || !valid_file_name(&self.artifact_file_name)
            || !valid_digest(&self.credential_broker_digest)
            || !valid_digest(&self.cosign_digest)
            || self.max_artifact_bytes == 0
            || self.max_artifact_bytes > MAX_ARTIFACT_BYTES
            || self.max_evidence_bytes == 0
            || self.max_evidence_bytes > MAX_EVIDENCE_BYTES
            || self.publication_timeout_seconds == 0
            || self.publication_timeout_seconds > MAX_PUBLICATION_TIMEOUT_SECONDS
        {
            return Err(StaticDistributionPublisherError::InvalidConfig(
                "publisher config fields are invalid".to_string(),
            ));
        }
        CommandRegistryCredentialBroker::new(
            self.credential_broker_path.clone(),
            self.credential_broker_digest.clone(),
        )
        .map_err(StaticDistributionPublisherError::InvalidConfig)?;
        CosignArtifactSigner::new(
            self.cosign_path.clone(),
            self.cosign_digest.clone(),
            self.cosign_key_reference.clone(),
        )
        .map_err(StaticDistributionPublisherError::InvalidConfig)?;
        Ok(())
    }

    pub fn publication_target(&self) -> OciArtifactPublicationTarget {
        OciArtifactPublicationTarget {
            registry: self.registry.clone(),
            repository: self.repository.clone(),
        }
    }

    fn publication_timeout(&self) -> Duration {
        Duration::from_secs(self.publication_timeout_seconds)
    }
}

#[derive(Debug, Error)]
pub enum StaticDistributionPublisherError {
    #[error("static distribution publisher config is invalid: {0}")]
    InvalidConfig(String),
    #[error("static distribution publisher input is invalid: {0}")]
    InvalidInput(String),
    #[error("static distribution publisher operation failed: {0}")]
    Publication(String),
    #[error("static distribution publisher I/O failed: {0}")]
    Io(String),
}

#[derive(Serialize)]
struct StaticDistributionArtifactConfig<'a> {
    contract: &'static str,
    distribution_build_id: uuid::Uuid,
    claim_id: uuid::Uuid,
    attempt_number: u32,
    job_request_digest: &'a str,
    generated_output_digest: &'a str,
    composition_digest: &'a str,
    toolchain_digest: &'a str,
    build_target: &'a str,
    resolved_lock_digest: &'a str,
    artifact_digest: &'a str,
    role_set_digest: &'a str,
    roles: &'a [ModuleStaticDistributionRoleArtifact],
}

pub async fn run_static_distribution_publisher(
    paths: StaticDistributionPublisherPaths,
) -> Result<(), StaticDistributionPublisherError> {
    validate_paths(&paths)?;
    let config = StaticDistributionPublisherConfig::load(&paths.config, &paths.config_digest)?;
    let publication = publish(paths, config.clone());
    timeout(config.publication_timeout(), publication)
        .await
        .map_err(|_| {
            StaticDistributionPublisherError::Publication(
                "publisher exceeded its configured deadline".to_string(),
            )
        })?
}

async fn publish(
    paths: StaticDistributionPublisherPaths,
    config: StaticDistributionPublisherConfig,
) -> Result<(), StaticDistributionPublisherError> {
    let inputs = load_and_validate_publisher_inputs(&paths)?;
    let prepared = prepare_publication_artifact(&paths.workspace, &inputs.request, &config)?;
    let evidence = publish_and_sign_artifacts(&inputs, &prepared, &config).await?;
    write_publication_receipt(&paths, &inputs, &prepared, &evidence)
}

struct ValidatedPublisherInputs {
    request: StaticDistributionPublisherRequest,
    publisher_request_digest: String,
    test_evidence_bytes: Vec<u8>,
    test_evidence_digest: String,
    lock_bytes: Vec<u8>,
}

fn load_and_validate_publisher_inputs(
    paths: &StaticDistributionPublisherPaths,
) -> Result<ValidatedPublisherInputs, StaticDistributionPublisherError> {
    let request_bytes = read_bounded_regular(&paths.request, MAX_REQUEST_BYTES)?;
    let request: StaticDistributionPublisherRequest = serde_json::from_slice(&request_bytes)
        .map_err(|error| {
            StaticDistributionPublisherError::InvalidInput(format!(
                "publisher request JSON is invalid: {error}"
            ))
        })?;
    validate_request(&request)?;
    let publisher_request_digest = digest_bytes(&request_bytes);
    let (test_evidence_bytes, test_evidence_digest) =
        load_and_validate_test_evidence(&paths.test_evidence, &request)?;
    let lock_bytes = load_and_validate_workspace_artifacts(&paths.workspace, &request)?;
    Ok(ValidatedPublisherInputs {
        request,
        publisher_request_digest,
        test_evidence_bytes,
        test_evidence_digest,
        lock_bytes,
    })
}

fn load_and_validate_test_evidence(
    path: &Path,
    request: &StaticDistributionPublisherRequest,
) -> Result<(Vec<u8>, String), StaticDistributionPublisherError> {
    let bytes = read_bounded_regular(path, MAX_TEST_EVIDENCE_BYTES)?;
    let digest = digest_bytes(&bytes);
    let evidence: StaticDistributionTestEvidence =
        serde_json::from_slice(&bytes).map_err(|error| {
            StaticDistributionPublisherError::InvalidInput(format!(
                "test evidence JSON is invalid: {error}"
            ))
        })?;
    validate_test_evidence(&evidence, request, &digest)?;
    Ok((bytes, digest))
}

fn load_and_validate_workspace_artifacts(
    workspace: &Path,
    request: &StaticDistributionPublisherRequest,
) -> Result<Vec<u8>, StaticDistributionPublisherError> {
    let manifest_bytes = read_bounded_regular(
        &workspace.join(GENERATED_MANIFEST_PATH),
        MAX_MANIFEST_BYTES,
    )?;
    let manifest: GeneratedStaticDistributionManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| {
            StaticDistributionPublisherError::InvalidInput(format!(
                "generated distribution manifest is invalid: {error}"
            ))
        })?;
    validate_manifest(&manifest, request)?;

    let lock_bytes =
        read_bounded_regular(&workspace.join(WORKSPACE_LOCK_PATH), MAX_LOCK_BYTES)?;
    if digest_bytes(&lock_bytes) != request.resolved_lock_digest {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "resolved workspace lock does not match the publisher request".to_string(),
        ));
    }
    Ok(lock_bytes)
}

struct PreparedArtifact {
    artifact_bytes: Vec<u8>,
    artifact_digest: String,
    roles: Vec<ModuleStaticDistributionRoleArtifact>,
    role_set_digest: String,
    artifact_config_bytes: Vec<u8>,
}

fn prepare_publication_artifact(
    workspace: &Path,
    request: &StaticDistributionPublisherRequest,
    config: &StaticDistributionPublisherConfig,
) -> Result<PreparedArtifact, StaticDistributionPublisherError> {
    let artifact_path = workspace
        .join(".rustok")
        .join("target")
        .join(&request.build_target)
        .join("release")
        .join(&config.artifact_file_name);
    let artifact_bytes = read_bounded_regular(&artifact_path, config.max_artifact_bytes)?;
    let artifact_digest = digest_bytes(&artifact_bytes);
    let roles = canonical_role_artifacts(&artifact_digest);
    let role_set_digest = ModuleStaticDistributionBuildEvidence::role_set_digest(&roles)
        .map_err(|error| StaticDistributionPublisherError::InvalidInput(error.to_string()))?;
    let artifact_config_bytes = serialize_artifact_config(
        request,
        &artifact_digest,
        &role_set_digest,
        &roles,
        config.max_evidence_bytes,
    )?;
    Ok(PreparedArtifact {
        artifact_bytes,
        artifact_digest,
        roles,
        role_set_digest,
        artifact_config_bytes,
    })
}

fn serialize_artifact_config(
    request: &StaticDistributionPublisherRequest,
    artifact_digest: &str,
    role_set_digest: &str,
    roles: &[ModuleStaticDistributionRoleArtifact],
    max_evidence_bytes: u64,
) -> Result<Vec<u8>, StaticDistributionPublisherError> {
    let artifact_config = StaticDistributionArtifactConfig {
        contract: ARTIFACT_CONFIG_CONTRACT,
        distribution_build_id: request.distribution_build_id,
        claim_id: request.claim_id,
        attempt_number: request.attempt_number,
        job_request_digest: &request.job_request_digest,
        generated_output_digest: &request.generated_output_digest,
        composition_digest: &request.composition_digest,
        toolchain_digest: &request.toolchain_digest,
        build_target: &request.build_target,
        resolved_lock_digest: &request.resolved_lock_digest,
        artifact_digest,
        role_set_digest,
        roles,
    };
    let bytes = serde_json::to_vec_pretty(&artifact_config)
        .map_err(|error| StaticDistributionPublisherError::Io(error.to_string()))?;
    if bytes.len() as u64 > max_evidence_bytes {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "artifact config exceeds the evidence bound".to_string(),
        ));
    }
    Ok(bytes)
}

struct PublishedArtifactEvidence {
    artifact: rustok_modules::OciArtifactReference,
    sbom: rustok_modules::OciArtifactReference,
    provenance: rustok_modules::OciArtifactReference,
    test_evidence_reference: rustok_modules::OciArtifactReference,
    signature: rustok_modules::OciArtifactReference,
}

async fn publish_and_sign_artifacts(
    inputs: &ValidatedPublisherInputs,
    prepared: &PreparedArtifact,
    config: &StaticDistributionPublisherConfig,
) -> Result<PublishedArtifactEvidence, StaticDistributionPublisherError> {
    let target = config.publication_target();
    let (publisher, signer, credentials) = setup_publisher_and_signer(config, &target).await?;
    let artifact =
        publish_primary_artifact(&publisher, &target, prepared, config.max_artifact_bytes).await?;
    let (sbom, provenance, test_evidence_reference) =
        publish_referrers(&publisher, &target, &artifact, inputs, config).await?;
    let signature = sign_and_resolve_signature(
        &publisher,
        &signer,
        &target,
        &artifact,
        &credentials,
        config.publication_timeout(),
    )
    .await?;

    Ok(PublishedArtifactEvidence {
        artifact,
        sbom,
        provenance,
        test_evidence_reference,
        signature,
    })
}

async fn publish_primary_artifact(
    publisher: &OciDistributionArtifactPublisher,
    target: &OciArtifactPublicationTarget,
    prepared: &PreparedArtifact,
    max_artifact_bytes: u64,
) -> Result<rustok_modules::OciArtifactReference, StaticDistributionPublisherError> {
    publisher
        .publish_build_artifact(
            target,
            OciBuildPublicationArtifact {
                config: publication_blob(
                    ARTIFACT_CONFIG_MEDIA_TYPE,
                    prepared.artifact_config_bytes.clone(),
                ),
                layer: OciBuildPublicationBlob {
                    media_type: ARTIFACT_LAYER_MEDIA_TYPE.to_string(),
                    digest: prepared.artifact_digest.clone(),
                    bytes: prepared.artifact_bytes.clone(),
                },
            },
            max_artifact_bytes,
        )
        .await
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))
}

async fn sign_and_resolve_signature(
    publisher: &OciDistributionArtifactPublisher,
    signer: &CosignArtifactSigner,
    target: &OciArtifactPublicationTarget,
    artifact: &rustok_modules::OciArtifactReference,
    credentials: &rustok_build_publication::RegistryCredentialLease,
    timeout: Duration,
) -> Result<rustok_modules::OciArtifactReference, StaticDistributionPublisherError> {
    credentials
        .ensure_valid()
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))?;
    signer
        .sign(artifact, credentials, timeout)
        .await
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))?;
    publisher
        .resolve_cosign_signature(target, artifact)
        .await
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))
}

async fn setup_publisher_and_signer(
    config: &StaticDistributionPublisherConfig,
    target: &OciArtifactPublicationTarget,
) -> Result<
    (
        OciDistributionArtifactPublisher,
        CosignArtifactSigner,
        rustok_build_publication::RegistryCredentialLease,
    ),
    StaticDistributionPublisherError,
> {
    let credential_broker = CommandRegistryCredentialBroker::new(
        config.credential_broker_path.clone(),
        config.credential_broker_digest.clone(),
    )
    .map_err(StaticDistributionPublisherError::InvalidConfig)?;
    let signer = CosignArtifactSigner::new(
        config.cosign_path.clone(),
        config.cosign_digest.clone(),
        config.cosign_key_reference.clone(),
    )
    .map_err(StaticDistributionPublisherError::InvalidConfig)?;
    let credentials = credential_broker
        .acquire(
            target,
            config
                .publication_timeout()
                .saturating_add(CREDENTIAL_SAFETY_MARGIN),
        )
        .await
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))?;
    credentials
        .ensure_valid()
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))?;
    let publisher = OciDistributionArtifactPublisher::strict(credentials.registry_auth())
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))?;
    Ok((publisher, signer, credentials))
}

type PublishedReferrers = (
    rustok_modules::OciArtifactReference,
    rustok_modules::OciArtifactReference,
    rustok_modules::OciArtifactReference,
);

async fn publish_referrers(
    publisher: &OciDistributionArtifactPublisher,
    target: &OciArtifactPublicationTarget,
    artifact: &rustok_modules::OciArtifactReference,
    inputs: &ValidatedPublisherInputs,
    config: &StaticDistributionPublisherConfig,
) -> Result<PublishedReferrers, StaticDistributionPublisherError> {
    let sbom = publish_sbom_referrer(
        publisher,
        target,
        artifact,
        &inputs.request,
        &inputs.lock_bytes,
        config.max_evidence_bytes,
    )
    .await?;
    let provenance = publish_provenance_referrer(
        publisher,
        target,
        artifact,
        &inputs.request,
        &inputs.publisher_request_digest,
        config.max_evidence_bytes,
    )
    .await?;
    let test_evidence = publish_test_evidence_referrer(
        publisher,
        target,
        artifact,
        inputs.test_evidence_digest.clone(),
        inputs.test_evidence_bytes.clone(),
        config.max_evidence_bytes,
    )
    .await?;
    Ok((sbom, provenance, test_evidence))
}

async fn publish_sbom_referrer(
    publisher: &OciDistributionArtifactPublisher,
    target: &OciArtifactPublicationTarget,
    artifact: &rustok_modules::OciArtifactReference,
    request: &StaticDistributionPublisherRequest,
    lock_bytes: &[u8],
    max_evidence_bytes: u64,
) -> Result<rustok_modules::OciArtifactReference, StaticDistributionPublisherError> {
    let sbom_bytes = build_cyclonedx_sbom(request, lock_bytes, max_evidence_bytes)?;
    publisher
        .publish_build_referrer(
            target,
            artifact,
            publication_blob(SBOM_MEDIA_TYPE, sbom_bytes),
            max_evidence_bytes,
        )
        .await
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))
}

async fn publish_test_evidence_referrer(
    publisher: &OciDistributionArtifactPublisher,
    target: &OciArtifactPublicationTarget,
    artifact: &rustok_modules::OciArtifactReference,
    test_evidence_digest: String,
    test_evidence_bytes: Vec<u8>,
    max_evidence_bytes: u64,
) -> Result<rustok_modules::OciArtifactReference, StaticDistributionPublisherError> {
    publisher
        .publish_build_referrer(
            target,
            artifact,
            OciBuildPublicationBlob {
                media_type: TEST_EVIDENCE_MEDIA_TYPE.to_string(),
                digest: test_evidence_digest,
                bytes: test_evidence_bytes,
            },
            max_evidence_bytes,
        )
        .await
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))
}

async fn publish_provenance_referrer(
    publisher: &OciDistributionArtifactPublisher,
    target: &OciArtifactPublicationTarget,
    artifact: &rustok_modules::OciArtifactReference,
    request: &StaticDistributionPublisherRequest,
    publisher_request_digest: &str,
    max_evidence_bytes: u64,
) -> Result<rustok_modules::OciArtifactReference, StaticDistributionPublisherError> {
    let provenance_bytes = build_slsa_provenance(request, artifact, publisher_request_digest)?;
    if provenance_bytes.len() as u64 > max_evidence_bytes {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "provenance exceeds the evidence bound".to_string(),
        ));
    }
    publisher
        .publish_build_referrer(
            target,
            artifact,
            publication_blob(PROVENANCE_MEDIA_TYPE, provenance_bytes),
            max_evidence_bytes,
        )
        .await
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))
}

fn write_publication_receipt(
    paths: &StaticDistributionPublisherPaths,
    inputs: &ValidatedPublisherInputs,
    prepared: &PreparedArtifact,
    evidence: &PublishedArtifactEvidence,
) -> Result<(), StaticDistributionPublisherError> {
    let receipt = StaticDistributionPublicationReceipt {
        contract: PUBLICATION_RECEIPT_CONTRACT.to_string(),
        publisher_request_digest: inputs.publisher_request_digest.clone(),
        job_request_digest: inputs.request.job_request_digest.clone(),
        generated_output_digest: inputs.request.generated_output_digest.clone(),
        composition_digest: inputs.request.composition_digest.clone(),
        resolved_lock_digest: inputs.request.resolved_lock_digest.clone(),
        test_evidence_payload_digest: inputs.request.test_evidence_digest.clone(),
        evidence: ModuleStaticDistributionBuildEvidence {
            bundle_reference: evidence.artifact.canonical(),
            bundle_root_digest: evidence.artifact.digest.clone(),
            role_set_digest: prepared.role_set_digest.clone(),
            roles: prepared.roles.clone(),
            sbom_reference: evidence.sbom.canonical(),
            sbom_digest: evidence.sbom.digest.clone(),
            provenance_reference: evidence.provenance.canonical(),
            provenance_digest: evidence.provenance.digest.clone(),
            signature_reference: evidence.signature.canonical(),
            signature_digest: evidence.signature.digest.clone(),
            test_evidence_reference: evidence.test_evidence_reference.canonical(),
            test_evidence_digest: evidence.test_evidence_reference.digest.clone(),
        },
    };
    let receipt_bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| StaticDistributionPublisherError::Io(error.to_string()))?;
    write_new_file(&paths.receipt, &receipt_bytes)
}

fn canonical_role_artifacts(artifact_digest: &str) -> Vec<ModuleStaticDistributionRoleArtifact> {
    [
        ModuleStaticDistributionRole::Monolith,
        ModuleStaticDistributionRole::Api,
        ModuleStaticDistributionRole::AdminSsr,
        ModuleStaticDistributionRole::StorefrontSsr,
        ModuleStaticDistributionRole::Worker,
        ModuleStaticDistributionRole::Registry,
    ]
    .into_iter()
    .map(|role| ModuleStaticDistributionRoleArtifact {
        role,
        artifact_digest: artifact_digest.to_string(),
    })
    .collect()
}

fn validate_request(
    request: &StaticDistributionPublisherRequest,
) -> Result<(), StaticDistributionPublisherError> {
    if request.contract != PUBLISHER_REQUEST_CONTRACT
        || request.distribution_build_id.is_nil()
        || request.claim_id.is_nil()
        || request.attempt_number == 0
        || !valid_digest(&request.job_request_digest)
        || !valid_digest(&request.generated_output_digest)
        || !valid_digest(&request.composition_digest)
        || !valid_digest(&request.toolchain_digest)
        || !valid_build_target(&request.build_target)
        || !valid_digest(&request.resolved_lock_digest)
        || !valid_digest(&request.test_evidence_digest)
    {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "publisher request fields are invalid".to_string(),
        ));
    }
    Ok(())
}

fn validate_test_evidence(
    evidence: &StaticDistributionTestEvidence,
    request: &StaticDistributionPublisherRequest,
    evidence_digest: &str,
) -> Result<(), StaticDistributionPublisherError> {
    if evidence.contract != TEST_EVIDENCE_CONTRACT
        || evidence_digest != request.test_evidence_digest
        || evidence.job_request_digest != request.job_request_digest
        || evidence.generated_output_digest != request.generated_output_digest
        || evidence.composition_digest != request.composition_digest
        || evidence.toolchain_digest != request.toolchain_digest
        || evidence.build_target != request.build_target
        || evidence.resolved_lock_digest != request.resolved_lock_digest
        || evidence.lock_command != fixed_lock_command()
        || evidence.test_command != fixed_test_command(&request.build_target)
        || evidence.build_command != fixed_build_command(&request.build_target)
        || !valid_digest(&evidence.cargo_digest)
        || !valid_digest(&evidence.rustc_digest)
        || !evidence.tests_passed
        || !evidence.build_succeeded
    {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "test evidence does not match the publisher request".to_string(),
        ));
    }
    Ok(())
}

fn validate_manifest(
    manifest: &GeneratedStaticDistributionManifest,
    request: &StaticDistributionPublisherRequest,
) -> Result<(), StaticDistributionPublisherError> {
    if manifest.distribution_build_id != request.distribution_build_id
        || manifest.claim_id != request.claim_id
        || manifest.attempt_number != request.attempt_number
        || manifest.composition_digest != request.composition_digest
        || manifest.output_digest != request.generated_output_digest
        || manifest.toolchain_digest != request.toolchain_digest
        || manifest.build_target != request.build_target
    {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "generated manifest does not match the publisher request".to_string(),
        ));
    }
    Ok(())
}



fn publication_blob(media_type: &str, bytes: Vec<u8>) -> OciBuildPublicationBlob {
    OciBuildPublicationBlob {
        media_type: media_type.to_string(),
        digest: digest_bytes(&bytes),
        bytes,
    }
}

fn fixed_test_command(target: &str) -> Vec<String> {
    [
        "test",
        "--locked",
        "--offline",
        "--workspace",
        "--all-targets",
        "--target",
        target,
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn fixed_lock_command() -> Vec<String> {
    ["generate-lockfile", "--offline"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn fixed_build_command(target: &str) -> Vec<String> {
    [
        "build",
        "--locked",
        "--offline",
        "--workspace",
        "--release",
        "--target",
        target,
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn validate_paths(
    paths: &StaticDistributionPublisherPaths,
) -> Result<(), StaticDistributionPublisherError> {
    validate_absolute_paths(&[
        &paths.request,
        &paths.workspace,
        &paths.test_evidence,
        &paths.config,
        &paths.receipt,
    ])?;
    let metadata = fs::symlink_metadata(&paths.workspace).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "publisher workspace must be a non-symlink directory".to_string(),
        ));
    }
    if !valid_digest(&paths.config_digest) {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "publisher config digest is invalid".to_string(),
        ));
    }
    validate_attempt_containment(
        &paths.workspace,
        &paths.request,
        &paths.test_evidence,
        &paths.receipt,
    )?;
    match fs::symlink_metadata(&paths.receipt) {
        Ok(_) => Err(StaticDistributionPublisherError::InvalidInput(
            "publisher receipt already exists".to_string(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
}

fn validate_absolute_paths(paths: &[&Path]) -> Result<(), StaticDistributionPublisherError> {
    for path in paths {
        if !path.is_absolute() {
            return Err(StaticDistributionPublisherError::InvalidInput(
                "publisher paths must be absolute".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_attempt_containment(
    workspace: &Path,
    request: &Path,
    test_evidence: &Path,
    receipt: &Path,
) -> Result<(), StaticDistributionPublisherError> {
    let canonical_ws = fs::canonicalize(workspace).map_err(io_error)?;
    let job_dir = canonical_ws.parent().ok_or_else(|| {
        StaticDistributionPublisherError::InvalidInput(
            "publisher workspace has no attempt directory".to_string(),
        )
    })?;
    for path in [request, test_evidence] {
        let canonical = fs::canonicalize(path).map_err(io_error)?;
        if canonical.parent() != Some(job_dir) {
            return Err(StaticDistributionPublisherError::InvalidInput(
                "publisher input escaped its attempt directory".to_string(),
            ));
        }
    }
    if receipt.parent() != Some(job_dir) {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "publisher receipt escaped its attempt directory".to_string(),
        ));
    }
    Ok(())
}

fn read_bounded_regular(
    path: &Path,
    maximum_bytes: u64,
) -> Result<Vec<u8>, StaticDistributionPublisherError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > maximum_bytes
    {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "publisher input is not a bounded regular file".to_string(),
        ));
    }
    let mut file = fs::File::open(path).map_err(io_error)?;
    let capacity = usize::try_from(metadata.len()).map_err(|_| {
        StaticDistributionPublisherError::InvalidInput(
            "publisher input length cannot be represented on this platform".to_string(),
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.read_to_end(&mut bytes).map_err(io_error)?;
    if bytes.len() as u64 != metadata.len() {
        return Err(StaticDistributionPublisherError::Io(
            "publisher input changed while being read".to_string(),
        ));
    }
    Ok(bytes)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), StaticDistributionPublisherError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(io_error)?;
    file.write_all(bytes).map_err(io_error)?;
    file.sync_all().map_err(io_error)
}

pub(super) fn digest_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_file_name(value: &str) -> bool {
    let path = Path::new(value);
    valid_text(value, 255)
        && path.components().count() == 1
        && matches!(path.components().next(), Some(Component::Normal(_)))
}

fn valid_build_target(value: &str) -> bool {
    valid_text(value, 128)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && !value.starts_with('.')
        && !value.ends_with('.')
}

fn valid_text(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= maximum_bytes
        && !value.chars().any(char::is_control)
}

fn io_error(error: impl std::fmt::Display) -> StaticDistributionPublisherError {
    StaticDistributionPublisherError::Io(error.to_string())
}
