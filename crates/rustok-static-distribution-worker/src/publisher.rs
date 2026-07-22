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
    ModuleStaticDistributionBuildEvidence, OciArtifactPublicationTarget,
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
    let request_bytes = read_bounded_regular(&paths.request, MAX_REQUEST_BYTES)?;
    let request: StaticDistributionPublisherRequest = serde_json::from_slice(&request_bytes)
        .map_err(|error| {
            StaticDistributionPublisherError::InvalidInput(format!(
                "publisher request JSON is invalid: {error}"
            ))
        })?;
    validate_request(&request)?;
    let publisher_request_digest = digest_bytes(&request_bytes);

    let test_evidence_bytes = read_bounded_regular(&paths.test_evidence, MAX_TEST_EVIDENCE_BYTES)?;
    let test_evidence_digest = digest_bytes(&test_evidence_bytes);
    let test_evidence: StaticDistributionTestEvidence =
        serde_json::from_slice(&test_evidence_bytes).map_err(|error| {
            StaticDistributionPublisherError::InvalidInput(format!(
                "test evidence JSON is invalid: {error}"
            ))
        })?;
    validate_test_evidence(&test_evidence, &request, &test_evidence_digest)?;

    let manifest_bytes = read_bounded_regular(
        &paths.workspace.join(GENERATED_MANIFEST_PATH),
        MAX_MANIFEST_BYTES,
    )?;
    let manifest: GeneratedStaticDistributionManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| {
            StaticDistributionPublisherError::InvalidInput(format!(
                "generated distribution manifest is invalid: {error}"
            ))
        })?;
    validate_manifest(&manifest, &request)?;

    let lock_bytes =
        read_bounded_regular(&paths.workspace.join(WORKSPACE_LOCK_PATH), MAX_LOCK_BYTES)?;
    if digest_bytes(&lock_bytes) != request.resolved_lock_digest {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "resolved workspace lock does not match the publisher request".to_string(),
        ));
    }
    let artifact_path = paths
        .workspace
        .join(".rustok")
        .join("target")
        .join(&request.build_target)
        .join("release")
        .join(&config.artifact_file_name);
    let artifact_bytes = read_bounded_regular(&artifact_path, config.max_artifact_bytes)?;
    let artifact_digest = digest_bytes(&artifact_bytes);
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
        artifact_digest: &artifact_digest,
    };
    let artifact_config_bytes = serde_json::to_vec_pretty(&artifact_config)
        .map_err(|error| StaticDistributionPublisherError::Io(error.to_string()))?;
    if artifact_config_bytes.len() as u64 > config.max_evidence_bytes {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "artifact config exceeds the evidence bound".to_string(),
        ));
    }

    let target = config.publication_target();
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
            &target,
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
    let artifact = publisher
        .publish_build_artifact(
            &target,
            OciBuildPublicationArtifact {
                config: publication_blob(ARTIFACT_CONFIG_MEDIA_TYPE, artifact_config_bytes),
                layer: OciBuildPublicationBlob {
                    media_type: ARTIFACT_LAYER_MEDIA_TYPE.to_string(),
                    digest: artifact_digest,
                    bytes: artifact_bytes,
                },
            },
            config.max_artifact_bytes,
        )
        .await
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))?;

    let sbom_bytes = build_cyclonedx_sbom(&request, &lock_bytes, config.max_evidence_bytes)?;
    let sbom = publisher
        .publish_build_referrer(
            &target,
            &artifact,
            publication_blob(SBOM_MEDIA_TYPE, sbom_bytes),
            config.max_evidence_bytes,
        )
        .await
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))?;
    let provenance_bytes = build_slsa_provenance(&request, &artifact, &publisher_request_digest)?;
    if provenance_bytes.len() as u64 > config.max_evidence_bytes {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "provenance exceeds the evidence bound".to_string(),
        ));
    }
    let provenance = publisher
        .publish_build_referrer(
            &target,
            &artifact,
            publication_blob(PROVENANCE_MEDIA_TYPE, provenance_bytes),
            config.max_evidence_bytes,
        )
        .await
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))?;
    let test_evidence_reference = publisher
        .publish_build_referrer(
            &target,
            &artifact,
            OciBuildPublicationBlob {
                media_type: TEST_EVIDENCE_MEDIA_TYPE.to_string(),
                digest: test_evidence_digest,
                bytes: test_evidence_bytes,
            },
            config.max_evidence_bytes,
        )
        .await
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))?;
    credentials
        .ensure_valid()
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))?;
    signer
        .sign(&artifact, &credentials, config.publication_timeout())
        .await
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))?;
    let signature = publisher
        .resolve_cosign_signature(&target, &artifact)
        .await
        .map_err(|error| StaticDistributionPublisherError::Publication(error.to_string()))?;

    let receipt = StaticDistributionPublicationReceipt {
        contract: PUBLICATION_RECEIPT_CONTRACT.to_string(),
        publisher_request_digest,
        job_request_digest: request.job_request_digest,
        generated_output_digest: request.generated_output_digest,
        composition_digest: request.composition_digest,
        resolved_lock_digest: request.resolved_lock_digest,
        test_evidence_payload_digest: request.test_evidence_digest,
        evidence: ModuleStaticDistributionBuildEvidence {
            artifact_reference: artifact.canonical(),
            artifact_digest: artifact.digest,
            sbom_reference: sbom.canonical(),
            sbom_digest: sbom.digest,
            provenance_reference: provenance.canonical(),
            provenance_digest: provenance.digest,
            signature_reference: signature.canonical(),
            signature_digest: signature.digest,
            test_evidence_reference: test_evidence_reference.canonical(),
            test_evidence_digest: test_evidence_reference.digest,
        },
    };
    let receipt_bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| StaticDistributionPublisherError::Io(error.to_string()))?;
    write_new_file(&paths.receipt, &receipt_bytes)
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

fn build_cyclonedx_sbom(
    request: &StaticDistributionPublisherRequest,
    lock_bytes: &[u8],
    maximum_bytes: u64,
) -> Result<Vec<u8>, StaticDistributionPublisherError> {
    let lock_text = std::str::from_utf8(lock_bytes).map_err(|_| {
        StaticDistributionPublisherError::InvalidInput(
            "resolved Cargo.lock is not UTF-8".to_string(),
        )
    })?;
    let lock = lock_text.parse::<toml::Table>().map_err(|error| {
        StaticDistributionPublisherError::InvalidInput(format!(
            "resolved Cargo.lock is invalid: {error}"
        ))
    })?;
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| {
            StaticDistributionPublisherError::InvalidInput(
                "resolved Cargo.lock has no packages".to_string(),
            )
        })?;
    let mut components = Vec::with_capacity(packages.len());
    for package in packages {
        let package = package.as_table().ok_or_else(|| {
            StaticDistributionPublisherError::InvalidInput(
                "resolved Cargo.lock package is invalid".to_string(),
            )
        })?;
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| {
                StaticDistributionPublisherError::InvalidInput(
                    "resolved Cargo.lock package name is missing".to_string(),
                )
            })?;
        let version = package
            .get("version")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| {
                StaticDistributionPublisherError::InvalidInput(
                    "resolved Cargo.lock package version is missing".to_string(),
                )
            })?;
        let source = package
            .get("source")
            .and_then(toml::Value::as_str)
            .unwrap_or("workspace");
        let bom_ref = digest_bytes(format!("{name}\0{version}\0{source}").as_bytes());
        let mut component = serde_json::json!({
            "type": "library",
            "bom-ref": bom_ref,
            "name": name,
            "version": version,
            "properties": [{ "name": "rustok:cargo:source", "value": source }]
        });
        if let Some(checksum) = package.get("checksum").and_then(toml::Value::as_str) {
            if checksum.len() == 64
                && checksum
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                component["hashes"] =
                    serde_json::json!([{ "alg": "SHA-256", "content": checksum }]);
            }
        }
        components.push(component);
    }
    components.sort_by(|left, right| left["bom-ref"].as_str().cmp(&right["bom-ref"].as_str()));
    let document = serde_json::json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.6",
        "serialNumber": format!("urn:uuid:{}", request.distribution_build_id),
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "bom-ref": request.composition_digest,
                "name": "rustok-static-distribution",
                "version": request.distribution_build_id.to_string()
            },
            "properties": [
                { "name": "rustok:composition_digest", "value": request.composition_digest },
                { "name": "rustok:resolved_lock_digest", "value": request.resolved_lock_digest }
            ]
        },
        "components": components
    });
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|error| StaticDistributionPublisherError::Io(error.to_string()))?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "generated SBOM exceeds the evidence bound".to_string(),
        ));
    }
    Ok(bytes)
}

fn build_slsa_provenance(
    request: &StaticDistributionPublisherRequest,
    artifact: &rustok_modules::OciArtifactReference,
    publisher_request_digest: &str,
) -> Result<Vec<u8>, StaticDistributionPublisherError> {
    let subject_digest = artifact.digest.strip_prefix("sha256:").ok_or_else(|| {
        StaticDistributionPublisherError::InvalidInput(
            "published artifact digest is invalid".to_string(),
        )
    })?;
    let document = serde_json::json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{
            "name": artifact.canonical(),
            "digest": { "sha256": subject_digest }
        }],
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": "https://rustok.dev/build-types/static-distribution",
                "externalParameters": {
                    "distribution_build_id": request.distribution_build_id,
                    "composition_digest": request.composition_digest,
                    "generated_output_digest": request.generated_output_digest
                },
                "internalParameters": {
                    "job_request_digest": request.job_request_digest,
                    "publisher_request_digest": publisher_request_digest,
                    "toolchain_digest": request.toolchain_digest,
                    "build_target": request.build_target,
                    "resolved_lock_digest": request.resolved_lock_digest
                },
                "resolvedDependencies": []
            },
            "runDetails": {
                "builder": { "id": "https://rustok.dev/builders/static-distribution" },
                "metadata": { "invocationId": request.claim_id }
            }
        }
    });
    serde_json::to_vec_pretty(&document)
        .map_err(|error| StaticDistributionPublisherError::Io(error.to_string()))
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
    for path in [
        &paths.request,
        &paths.workspace,
        &paths.test_evidence,
        &paths.config,
        &paths.receipt,
    ] {
        if !path.is_absolute() {
            return Err(StaticDistributionPublisherError::InvalidInput(
                "publisher paths must be absolute".to_string(),
            ));
        }
    }
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
    let workspace = fs::canonicalize(&paths.workspace).map_err(io_error)?;
    let job_dir = workspace.parent().ok_or_else(|| {
        StaticDistributionPublisherError::InvalidInput(
            "publisher workspace has no attempt directory".to_string(),
        )
    })?;
    for path in [&paths.request, &paths.test_evidence] {
        let canonical = fs::canonicalize(path).map_err(io_error)?;
        if canonical.parent() != Some(job_dir) {
            return Err(StaticDistributionPublisherError::InvalidInput(
                "publisher input escaped its attempt directory".to_string(),
            ));
        }
    }
    if paths.receipt.parent() != Some(job_dir) {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "publisher receipt escaped its attempt directory".to_string(),
        ));
    }
    match fs::symlink_metadata(&paths.receipt) {
        Ok(_) => Err(StaticDistributionPublisherError::InvalidInput(
            "publisher receipt already exists".to_string(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
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

fn digest_bytes(bytes: &[u8]) -> String {
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
