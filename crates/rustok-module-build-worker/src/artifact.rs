use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use rustok_modules::{
    ArtifactAdmissionLimits, MAX_MODULE_ARTIFACT_SOURCE_MANIFEST_BYTES,
    MODULE_ARTIFACT_SOURCE_MANIFEST_FILE, ModuleArtifactDescriptor, ModuleArtifactSourceManifest,
    ModuleBuildComponentInterface, ModuleBuildRequest, ModuleBuildResult, OciArtifactEvidence,
    OciArtifactPublicationBundle,
};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    time::timeout,
};
use wasmparser::{Encoding, Parser, Payload, Validator};
use wit_parser::{Resolve, WorldKey};

const COMPONENT_OUTPUT_FILE: &str = "component.wasm";
const SBOM_OUTPUT_FILE: &str = "sbom.cdx.json";
const PROVENANCE_OUTPUT_FILE: &str = "provenance.intoto.json";
const DESCRIPTOR_OUTPUT_FILE: &str = "module-artifact-descriptor.json";
const MAX_COMPONENT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EVIDENCE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_INTERFACE_NAME_BYTES: usize = 256;
const MAX_SBOM_COMPONENTS: usize = 4_096;
const MAX_PROVENANCE_SUBJECTS: usize = 128;

struct BuildEvidenceExpectation {
    component_digest: String,
    sbom_digest: String,
    provenance_digest: String,
    source_digest: String,
    dependency_lock_digest: String,
    toolchain_digest: String,
    wit_digest: String,
    sdk_version: String,
    template_version: String,
    expected_module_slug: String,
    expected_version: String,
    runtime_abi: String,
    attempt: u32,
    validation_profiles: serde_json::Value,
    validation_results: serde_json::Value,
}

/// A terminal fact about the fixed runner's component output.
#[derive(Debug)]
pub enum ComponentArtifactError {
    InspectionFailed,
    ResourceLimit,
    Internal(String),
}

/// A terminal fact about fixed SBOM and provenance evidence emitted alongside
/// a successful component.
#[derive(Debug)]
pub enum BuildEvidenceError {
    SbomInvalid,
    ProvenanceInvalid,
    ResourceLimit,
    Internal(String),
}

/// A terminal fact that the Component's encoded WIT contract does not match
/// the immutable request or declares a capability outside that world.
#[derive(Debug)]
pub enum WitContractError {
    Mismatch,
    ResourceLimit,
    Internal(String),
}

/// A terminal fact that the runner's fixed output directory cannot provide an
/// immutable OCI publication bundle bound to the verified build result.
#[derive(Debug)]
pub enum PublicationBundleError {
    Invalid,
    ResourceLimit,
    Internal(String),
}

/// A terminal fact about the author source manifest or worker-owned descriptor
/// finalization step.
#[derive(Debug)]
pub enum ArtifactDescriptorError {
    Invalid,
    ResourceLimit,
    Internal(String),
}

/// Validates the one fixed component payload path produced by the image-owned
/// runner. The runner cannot select another path through JSON or request data.
pub struct ComponentArtifactInspector;

/// Validates the fixed evidence file names and request-bound provenance facts.
pub struct BuildEvidenceInspector;

/// Extracts WIT from validated Component bytes through a fixed image-owned
/// wasm-tools executable. No runner-owned WIT text or request-selected tool
/// path participates in this check.
pub struct WitContractInspector {
    wasm_tools_path: PathBuf,
}

/// Collects the fixed descriptor, payload, SBOM, and provenance files only
/// after their independent inspection stages have succeeded.
pub struct PublicationBundleCollector;

/// Holds a validated source declaration across the untrusted runner call and
/// writes the final descriptor only after component inspection succeeds.
pub struct ArtifactDescriptorFinalizer {
    source: ModuleArtifactSourceManifest,
}

impl ArtifactDescriptorFinalizer {
    pub async fn load(
        source_dir: &Path,
        request: &ModuleBuildRequest,
    ) -> Result<Self, ArtifactDescriptorError> {
        let source_dir = source_dir.to_path_buf();
        let request = request.clone();
        tokio::task::spawn_blocking(move || load_source_manifest(&source_dir, &request))
            .await
            .map_err(|error| {
                ArtifactDescriptorError::Internal(format!(
                    "artifact source manifest inspection failed: {error}"
                ))
            })?
            .map(|source| Self { source })
    }

    pub async fn finalize(
        self,
        output_dir: &Path,
        result: &ModuleBuildResult,
    ) -> Result<(), ArtifactDescriptorError> {
        let output_dir = output_dir.to_path_buf();
        let component_digest = result
            .component_digest
            .clone()
            .ok_or(ArtifactDescriptorError::Invalid)?;
        tokio::task::spawn_blocking(move || {
            finalize_descriptor(self.source, &output_dir, component_digest)
        })
        .await
        .map_err(|error| {
            ArtifactDescriptorError::Internal(format!(
                "artifact descriptor finalization failed: {error}"
            ))
        })?
    }
}

impl ComponentArtifactInspector {
    pub async fn inspect(
        output_dir: &Path,
        request: &ModuleBuildRequest,
        result: &ModuleBuildResult,
    ) -> Result<(), ComponentArtifactError> {
        let output_dir = output_dir.to_path_buf();
        let component_digest = result
            .component_digest
            .clone()
            .ok_or(ComponentArtifactError::InspectionFailed)?;
        let component_interface = result
            .component_interface
            .clone()
            .ok_or(ComponentArtifactError::InspectionFailed)?;
        let maximum_bytes = request
            .limits
            .disk_bytes
            .min(request.limits.memory_bytes / 4)
            .min(MAX_COMPONENT_BYTES);
        tokio::task::spawn_blocking(move || {
            inspect_component(
                &output_dir,
                &component_digest,
                &component_interface,
                maximum_bytes,
            )
        })
        .await
        .map_err(|error| {
            ComponentArtifactError::Internal(format!("component inspection failed: {error}"))
        })?
    }
}

impl BuildEvidenceInspector {
    pub async fn inspect(
        output_dir: &Path,
        request: &ModuleBuildRequest,
        result: &ModuleBuildResult,
    ) -> Result<(), BuildEvidenceError> {
        let output_dir = output_dir.to_path_buf();
        let expectation = BuildEvidenceExpectation {
            component_digest: result
                .component_digest
                .clone()
                .ok_or(BuildEvidenceError::ProvenanceInvalid)?,
            sbom_digest: result
                .sbom_digest
                .clone()
                .ok_or(BuildEvidenceError::SbomInvalid)?,
            provenance_digest: result
                .provenance_digest
                .clone()
                .ok_or(BuildEvidenceError::ProvenanceInvalid)?,
            source_digest: request.source.digest.clone(),
            dependency_lock_digest: request.dependency_policy.lock_digest.clone(),
            toolchain_digest: request.toolchain.protocol_digest(),
            wit_digest: request.wit.protocol_digest(),
            sdk_version: request.authoring.sdk_version.clone(),
            template_version: request.authoring.template_version.clone(),
            expected_module_slug: request.expected_module_slug.clone(),
            expected_version: request.expected_version.clone(),
            runtime_abi: request.runtime_abi.clone(),
            attempt: request.attempt,
            validation_profiles: serde_json::to_value(&request.validation_profiles)
                .map_err(|error| BuildEvidenceError::Internal(error.to_string()))?,
            validation_results: serde_json::to_value(&result.evidence.validation_results)
                .map_err(|error| BuildEvidenceError::Internal(error.to_string()))?,
        };
        let maximum_bytes = request
            .limits
            .disk_bytes
            .min(request.limits.memory_bytes / 4)
            .min(MAX_EVIDENCE_BYTES);
        tokio::task::spawn_blocking(move || {
            inspect_build_evidence(&output_dir, &expectation, maximum_bytes)
        })
        .await
        .map_err(|error| {
            BuildEvidenceError::Internal(format!("build evidence inspection failed: {error}"))
        })?
    }
}

impl WitContractInspector {
    pub fn new(wasm_tools_path: PathBuf) -> Result<Self, String> {
        if !wasm_tools_path.is_absolute() {
            return Err("module build wasm-tools path must be absolute".to_string());
        }
        let metadata = fs::symlink_metadata(&wasm_tools_path).map_err(|error| {
            format!(
                "module build wasm-tools {} cannot be inspected: {error}",
                wasm_tools_path.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("module build wasm-tools must be a regular non-symlink file".to_string());
        }
        Ok(Self { wasm_tools_path })
    }

    pub async fn inspect(
        &self,
        output_dir: &Path,
        request: &ModuleBuildRequest,
        result: &ModuleBuildResult,
        execution_timeout: Duration,
    ) -> Result<(), WitContractError> {
        let output_dir = output_dir.to_path_buf();
        let component_interface = result
            .component_interface
            .clone()
            .ok_or(WitContractError::Mismatch)?;
        let maximum_bytes = request
            .limits
            .disk_bytes
            .min(request.limits.memory_bytes / 4)
            .min(MAX_COMPONENT_BYTES);
        let expected_world = request.wit.world.clone();
        let expected_version = request.wit.version.clone();
        let component_path = output_dir.join(COMPONENT_OUTPUT_FILE);
        validate_component_input(&component_path, maximum_bytes)?;
        let wit = run_wasm_tools_wit(
            &self.wasm_tools_path,
            &component_path,
            request.limits.output_bytes,
            execution_timeout,
        )
        .await?;
        tokio::task::spawn_blocking(move || {
            inspect_wit_contract(
                &component_interface,
                &expected_world,
                &expected_version,
                &wit,
            )
        })
        .await
        .map_err(|error| {
            WitContractError::Internal(format!("WIT contract inspection failed: {error}"))
        })?
    }
}

impl PublicationBundleCollector {
    pub async fn collect(
        output_dir: &Path,
        request: &ModuleBuildRequest,
        result: &ModuleBuildResult,
    ) -> Result<OciArtifactPublicationBundle, PublicationBundleError> {
        let output_dir = output_dir.to_path_buf();
        let request = request.clone();
        let result = result.clone();
        tokio::task::spawn_blocking(move || {
            collect_publication_bundle(&output_dir, &request, &result)
        })
        .await
        .map_err(|error| {
            PublicationBundleError::Internal(format!(
                "publication bundle collection failed: {error}"
            ))
        })?
    }
}

fn load_source_manifest(
    source_dir: &Path,
    request: &ModuleBuildRequest,
) -> Result<ModuleArtifactSourceManifest, ArtifactDescriptorError> {
    let path = source_dir.join(MODULE_ARTIFACT_SOURCE_MANIFEST_FILE);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ArtifactDescriptorError::Invalid
        } else {
            ArtifactDescriptorError::Internal(format!(
                "artifact source manifest {} cannot be inspected: {error}",
                path.display()
            ))
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
        return Err(ArtifactDescriptorError::Invalid);
    }
    if metadata.len() > MAX_MODULE_ARTIFACT_SOURCE_MANIFEST_BYTES as u64 {
        return Err(ArtifactDescriptorError::ResourceLimit);
    }
    let bytes = fs::read(&path).map_err(|error| {
        ArtifactDescriptorError::Internal(format!(
            "artifact source manifest {} cannot be read: {error}",
            path.display()
        ))
    })?;
    let source = ModuleArtifactSourceManifest::parse(&bytes)
        .map_err(|_| ArtifactDescriptorError::Invalid)?;
    source
        .validate_build_request(request)
        .map_err(|_| ArtifactDescriptorError::Invalid)?;
    Ok(source)
}

fn finalize_descriptor(
    source: ModuleArtifactSourceManifest,
    output_dir: &Path,
    component_digest: String,
) -> Result<(), ArtifactDescriptorError> {
    let output_metadata = fs::symlink_metadata(output_dir).map_err(|error| {
        ArtifactDescriptorError::Internal(format!(
            "artifact output directory {} cannot be inspected: {error}",
            output_dir.display()
        ))
    })?;
    if output_metadata.file_type().is_symlink() || !output_metadata.is_dir() {
        return Err(ArtifactDescriptorError::Invalid);
    }
    let descriptor = source
        .finalize(component_digest)
        .map_err(|_| ArtifactDescriptorError::Invalid)?;
    let bytes = serde_json::to_vec(&descriptor).map_err(|error| {
        ArtifactDescriptorError::Internal(format!(
            "artifact descriptor cannot be serialized: {error}"
        ))
    })?;
    if bytes.len() > ArtifactAdmissionLimits::default().max_descriptor_bytes as usize {
        return Err(ArtifactDescriptorError::ResourceLimit);
    }
    let path = output_dir.join(DESCRIPTOR_OUTPUT_FILE);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                ArtifactDescriptorError::Invalid
            } else {
                ArtifactDescriptorError::Internal(format!(
                    "artifact descriptor {} cannot be created: {error}",
                    path.display()
                ))
            }
        })?;
    file.write_all(&bytes).map_err(|error| {
        ArtifactDescriptorError::Internal(format!(
            "artifact descriptor {} cannot be written: {error}",
            path.display()
        ))
    })?;
    file.sync_all().map_err(|error| {
        ArtifactDescriptorError::Internal(format!(
            "artifact descriptor {} cannot be synchronized: {error}",
            path.display()
        ))
    })
}

fn collect_publication_bundle(
    output_dir: &Path,
    request: &ModuleBuildRequest,
    result: &ModuleBuildResult,
) -> Result<OciArtifactPublicationBundle, PublicationBundleError> {
    let maximum_payload_bytes = request
        .limits
        .disk_bytes
        .min(request.limits.memory_bytes / 4)
        .min(MAX_COMPONENT_BYTES);
    let payload =
        read_regular_publication_output(output_dir, COMPONENT_OUTPUT_FILE, maximum_payload_bytes)?;
    let sbom =
        read_regular_publication_output(output_dir, SBOM_OUTPUT_FILE, maximum_payload_bytes)?;
    let provenance =
        read_regular_publication_output(output_dir, PROVENANCE_OUTPUT_FILE, maximum_payload_bytes)?;
    let descriptor_bytes = read_regular_publication_output(
        output_dir,
        DESCRIPTOR_OUTPUT_FILE,
        ArtifactAdmissionLimits::default().max_descriptor_bytes,
    )?;
    let descriptor: ModuleArtifactDescriptor =
        serde_json::from_slice(&descriptor_bytes).map_err(|_| PublicationBundleError::Invalid)?;
    OciArtifactPublicationBundle::from_verified_component(
        request,
        result,
        descriptor,
        payload,
        OciArtifactEvidence {
            digest: result
                .sbom_digest
                .clone()
                .ok_or(PublicationBundleError::Invalid)?,
            bytes: sbom,
        },
        OciArtifactEvidence {
            digest: result
                .provenance_digest
                .clone()
                .ok_or(PublicationBundleError::Invalid)?,
            bytes: provenance,
        },
        ArtifactAdmissionLimits {
            max_descriptor_bytes: ArtifactAdmissionLimits::default().max_descriptor_bytes,
            max_payload_bytes: maximum_payload_bytes,
        },
    )
    .map_err(|_| PublicationBundleError::Invalid)
}

fn read_regular_publication_output(
    output_dir: &Path,
    file_name: &str,
    maximum_bytes: u64,
) -> Result<Vec<u8>, PublicationBundleError> {
    let path = output_dir.join(file_name);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            PublicationBundleError::Invalid
        } else {
            PublicationBundleError::Internal(format!(
                "publication output {} cannot be inspected: {error}",
                path.display()
            ))
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
        return Err(PublicationBundleError::Invalid);
    }
    if metadata.len() > maximum_bytes {
        return Err(PublicationBundleError::ResourceLimit);
    }
    fs::read(&path).map_err(|error| {
        PublicationBundleError::Internal(format!(
            "publication output {} cannot be read: {error}",
            path.display()
        ))
    })
}

fn validate_component_input(path: &Path, maximum_bytes: u64) -> Result<(), WitContractError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            WitContractError::Mismatch
        } else {
            WitContractError::Internal(format!(
                "component WIT input {} cannot be inspected: {error}",
                path.display()
            ))
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
        return Err(WitContractError::Mismatch);
    }
    if metadata.len() > maximum_bytes {
        return Err(WitContractError::ResourceLimit);
    }
    Ok(())
}

async fn run_wasm_tools_wit(
    wasm_tools_path: &Path,
    component_path: &Path,
    output_limit: u64,
    execution_timeout: Duration,
) -> Result<String, WitContractError> {
    if execution_timeout.is_zero() {
        return Err(WitContractError::ResourceLimit);
    }
    let output_limit =
        usize::try_from(output_limit).map_err(|_| WitContractError::ResourceLimit)?;
    let budget = Arc::new(WitOutputBudget::new(output_limit));
    let mut child = Command::new(wasm_tools_path)
        .env_clear()
        .args(["component", "wit"])
        .arg(component_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| WitContractError::Internal(error.to_string()))?;
    let stdout = child.stdout.take().ok_or_else(|| {
        WitContractError::Internal("wasm-tools WIT stdout is unavailable".to_string())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        WitContractError::Internal("wasm-tools WIT stderr is unavailable".to_string())
    })?;
    let stdout_task = tokio::spawn(read_wit_output(stdout, Arc::clone(&budget), true));
    let stderr_task = tokio::spawn(read_wit_output(stderr, budget, false));
    let status = match timeout(execution_timeout, child.wait()).await {
        Ok(status) => status.map_err(|error| WitContractError::Internal(error.to_string()))?,
        Err(_) => {
            let _ = child.kill().await;
            stdout_task.abort();
            stderr_task.abort();
            return Err(WitContractError::ResourceLimit);
        }
    };
    let stdout = collect_wit_output(stdout_task).await?;
    collect_wit_output(stderr_task).await?;
    if !status.success() {
        return Err(WitContractError::Mismatch);
    }
    let wit = std::str::from_utf8(&stdout).map_err(|_| WitContractError::Mismatch)?;
    if wit.trim().is_empty() {
        return Err(WitContractError::Mismatch);
    }
    Ok(wit.to_owned())
}

struct WitOutputBudget {
    limit: usize,
    consumed: AtomicUsize,
}

impl WitOutputBudget {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            consumed: AtomicUsize::new(0),
        }
    }

    fn reserve(&self, bytes: usize) -> bool {
        let previous = self.consumed.fetch_add(bytes, Ordering::Relaxed);
        previous.saturating_add(bytes) <= self.limit
    }
}

async fn read_wit_output<R>(
    mut reader: R,
    budget: Arc<WitOutputBudget>,
    retain: bool,
) -> Result<Vec<u8>, WitContractError>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    let mut exceeded = false;
    loop {
        let read = reader
            .read(&mut buffer)
            .await
            .map_err(|error| WitContractError::Internal(error.to_string()))?;
        if read == 0 {
            return if exceeded {
                Err(WitContractError::ResourceLimit)
            } else {
                Ok(output)
            };
        }
        if !budget.reserve(read) {
            exceeded = true;
        } else if retain {
            output.extend_from_slice(&buffer[..read]);
        }
    }
}

async fn collect_wit_output(
    task: tokio::task::JoinHandle<Result<Vec<u8>, WitContractError>>,
) -> Result<Vec<u8>, WitContractError> {
    task.await.map_err(|error| {
        WitContractError::Internal(format!("wasm-tools WIT output reader failed: {error}"))
    })?
}

fn inspect_component(
    output_dir: &Path,
    expected_digest: &str,
    expected_interface: &ModuleBuildComponentInterface,
    maximum_bytes: u64,
) -> Result<(), ComponentArtifactError> {
    let component_path = output_dir.join(COMPONENT_OUTPUT_FILE);
    let metadata = fs::symlink_metadata(&component_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ComponentArtifactError::InspectionFailed
        } else {
            ComponentArtifactError::Internal(format!(
                "component output {} cannot be inspected: {error}",
                component_path.display()
            ))
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
        return Err(ComponentArtifactError::InspectionFailed);
    }
    if metadata.len() > maximum_bytes {
        return Err(ComponentArtifactError::ResourceLimit);
    }
    let bytes = fs::read(&component_path).map_err(|error| {
        ComponentArtifactError::Internal(format!(
            "component output {} cannot be read: {error}",
            component_path.display()
        ))
    })?;
    let actual_digest = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    if actual_digest != expected_digest {
        return Err(ComponentArtifactError::InspectionFailed);
    }
    Validator::new()
        .validate_all(&bytes)
        .map_err(|_| ComponentArtifactError::InspectionFailed)?;
    let actual_interface = inspect_component_interface(&bytes)?;
    if normalized_interface(expected_interface)? != actual_interface {
        return Err(ComponentArtifactError::InspectionFailed);
    }
    Ok(())
}

fn inspect_wit_contract(
    actual_interface: &ModuleBuildComponentInterface,
    expected_world: &str,
    expected_version: &str,
    wit: &str,
) -> Result<(), WitContractError> {
    let (expected_namespace, expected_package, expected_world_name) =
        parse_expected_world(expected_world)?;
    let mut resolve = Resolve::default();
    let package_id = resolve
        .push_str("component.wit", wit)
        .map_err(|_| WitContractError::Mismatch)?;
    let package = &resolve.packages[package_id];
    let world_id = package
        .worlds
        .get(&expected_world_name)
        .copied()
        .ok_or(WitContractError::Mismatch)?;
    let world = &resolve.worlds[world_id];
    if package.name.namespace != expected_namespace
        || package.name.name != expected_package
        || package
            .name
            .version
            .as_ref()
            .map(ToString::to_string)
            .as_deref()
            != Some(expected_version)
        || world.name != expected_world_name
    {
        return Err(WitContractError::Mismatch);
    }
    let declared_imports = world_surface_names(&resolve, world.imports.keys())?;
    let declared_exports = world_surface_names(&resolve, world.exports.keys())?;
    let actual = normalize_interface_for_wit(actual_interface)?;
    if actual.imports != declared_imports || actual.exports != declared_exports {
        return Err(WitContractError::Mismatch);
    }
    Ok(())
}

fn parse_expected_world(value: &str) -> Result<(String, String, String), WitContractError> {
    let (package, world) = value.rsplit_once('/').ok_or(WitContractError::Mismatch)?;
    let (namespace, package) = package.split_once(':').ok_or(WitContractError::Mismatch)?;
    if !valid_wit_identifier(namespace)
        || !valid_wit_identifier(package)
        || !valid_wit_identifier(world)
        || value.matches('/').count() != 1
        || value.matches(':').count() != 1
    {
        return Err(WitContractError::Mismatch);
    }
    Ok((namespace.to_owned(), package.to_owned(), world.to_owned()))
}

fn world_surface_names<'a>(
    resolve: &Resolve,
    keys: impl Iterator<Item = &'a WorldKey>,
) -> Result<Vec<String>, WitContractError> {
    let mut names = BTreeSet::new();
    for key in keys {
        let name = match key {
            WorldKey::Name(name) => name.clone(),
            WorldKey::Interface(interface_id) => {
                let interface = &resolve.interfaces[*interface_id];
                let interface_name = interface.name.as_ref().ok_or(WitContractError::Mismatch)?;
                let package_id = interface.package.ok_or(WitContractError::Mismatch)?;
                resolve.packages[package_id]
                    .name
                    .interface_id(interface_name)
            }
        };
        if !valid_interface_name(&name) || !names.insert(name) {
            return Err(WitContractError::Mismatch);
        }
    }
    Ok(names.into_iter().collect())
}

fn inspect_build_evidence(
    output_dir: &Path,
    expectation: &BuildEvidenceExpectation,
    maximum_bytes: u64,
) -> Result<(), BuildEvidenceError> {
    let sbom = read_verified_json_output(
        output_dir,
        SBOM_OUTPUT_FILE,
        &expectation.sbom_digest,
        maximum_bytes,
        BuildEvidenceError::SbomInvalid,
    )?;
    inspect_cyclonedx_sbom(&sbom)?;
    let provenance = read_verified_json_output(
        output_dir,
        PROVENANCE_OUTPUT_FILE,
        &expectation.provenance_digest,
        maximum_bytes,
        BuildEvidenceError::ProvenanceInvalid,
    )?;
    inspect_slsa_provenance(&provenance, expectation)
}

fn read_verified_json_output(
    output_dir: &Path,
    name: &str,
    expected_digest: &str,
    maximum_bytes: u64,
    invalid: BuildEvidenceError,
) -> Result<serde_json::Value, BuildEvidenceError> {
    let path = output_dir.join(name);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Err(invalid),
        Err(error) => {
            return Err(BuildEvidenceError::Internal(format!(
                "build evidence {} cannot be inspected: {error}",
                path.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
        return Err(invalid);
    }
    if metadata.len() > maximum_bytes {
        return Err(BuildEvidenceError::ResourceLimit);
    }
    let bytes = fs::read(&path).map_err(|error| {
        BuildEvidenceError::Internal(format!(
            "build evidence {} cannot be read: {error}",
            path.display()
        ))
    })?;
    if format!("sha256:{}", hex::encode(Sha256::digest(&bytes))) != expected_digest {
        return Err(invalid);
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid)
}

fn inspect_cyclonedx_sbom(document: &serde_json::Value) -> Result<(), BuildEvidenceError> {
    if document
        .get("bomFormat")
        .and_then(serde_json::Value::as_str)
        != Some("CycloneDX")
        || document
            .get("specVersion")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err(BuildEvidenceError::SbomInvalid);
    }
    let metadata_component = document
        .pointer("/metadata/component")
        .and_then(serde_json::Value::as_object);
    let components = document
        .get("components")
        .and_then(serde_json::Value::as_array);
    if metadata_component.is_none()
        || components.is_none_or(|components| components.len() > MAX_SBOM_COMPONENTS)
    {
        return Err(BuildEvidenceError::SbomInvalid);
    }
    Ok(())
}

fn inspect_slsa_provenance(
    document: &serde_json::Value,
    expectation: &BuildEvidenceExpectation,
) -> Result<(), BuildEvidenceError> {
    if !document
        .get("_type")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| value.starts_with("https://in-toto.io/Statement/"))
        || !document
            .get("predicateType")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value.starts_with("https://slsa.dev/provenance/"))
    {
        return Err(BuildEvidenceError::ProvenanceInvalid);
    }
    let expected_component_hash = expectation
        .component_digest
        .strip_prefix("sha256:")
        .ok_or(BuildEvidenceError::ProvenanceInvalid)?;
    let subjects = document
        .get("subject")
        .and_then(serde_json::Value::as_array)
        .filter(|subjects| !subjects.is_empty() && subjects.len() <= MAX_PROVENANCE_SUBJECTS)
        .ok_or(BuildEvidenceError::ProvenanceInvalid)?;
    if !subjects.iter().any(|subject| {
        subject
            .pointer("/digest/sha256")
            .and_then(serde_json::Value::as_str)
            == Some(expected_component_hash)
    }) {
        return Err(BuildEvidenceError::ProvenanceInvalid);
    }
    let rustok = document
        .pointer("/predicate/buildDefinition/externalParameters/rustok")
        .and_then(serde_json::Value::as_object)
        .ok_or(BuildEvidenceError::ProvenanceInvalid)?;
    for (key, expected) in [
        ("sourceDigest", expectation.source_digest.as_str()),
        (
            "dependencyLockDigest",
            expectation.dependency_lock_digest.as_str(),
        ),
        ("toolchainDigest", expectation.toolchain_digest.as_str()),
        ("witDigest", expectation.wit_digest.as_str()),
        ("sdkVersion", expectation.sdk_version.as_str()),
        ("templateVersion", expectation.template_version.as_str()),
        (
            "expectedModuleSlug",
            expectation.expected_module_slug.as_str(),
        ),
        ("expectedVersion", expectation.expected_version.as_str()),
        ("runtimeAbi", expectation.runtime_abi.as_str()),
    ] {
        if rustok.get(key).and_then(serde_json::Value::as_str) != Some(expected) {
            return Err(BuildEvidenceError::ProvenanceInvalid);
        }
    }
    if rustok.get("attempt").and_then(serde_json::Value::as_u64)
        != Some(u64::from(expectation.attempt))
        || rustok.get("validationProfiles") != Some(&expectation.validation_profiles)
        || rustok.get("validationResults") != Some(&expectation.validation_results)
    {
        return Err(BuildEvidenceError::ProvenanceInvalid);
    }
    if !document
        .pointer("/predicate/runDetails/builder/id")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| !value.is_empty())
    {
        return Err(BuildEvidenceError::ProvenanceInvalid);
    }
    Ok(())
}

fn inspect_component_interface(
    bytes: &[u8],
) -> Result<ModuleBuildComponentInterface, ComponentArtifactError> {
    let mut root_encoding = None;
    let mut depth = 0_usize;
    let mut imports = BTreeSet::new();
    let mut exports = BTreeSet::new();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.map_err(|_| ComponentArtifactError::InspectionFailed)? {
            Payload::Version { encoding, .. } if root_encoding.is_none() => {
                root_encoding = Some(encoding);
            }
            Payload::ModuleSection { .. } | Payload::ComponentSection { .. } => {
                depth = depth
                    .checked_add(1)
                    .ok_or(ComponentArtifactError::InspectionFailed)?;
            }
            Payload::ComponentImportSection(section) if depth == 0 => {
                for import in section {
                    let import = import.map_err(|_| ComponentArtifactError::InspectionFailed)?;
                    if !imports.insert(component_name(import.name)?) {
                        return Err(ComponentArtifactError::InspectionFailed);
                    }
                }
            }
            Payload::ComponentExportSection(section) if depth == 0 => {
                for export in section {
                    let export = export.map_err(|_| ComponentArtifactError::InspectionFailed)?;
                    if !exports.insert(component_name(export.name)?) {
                        return Err(ComponentArtifactError::InspectionFailed);
                    }
                }
            }
            Payload::End(_) if depth > 0 => depth -= 1,
            _ => {}
        }
    }
    if root_encoding != Some(Encoding::Component) || exports.is_empty() || depth != 0 {
        return Err(ComponentArtifactError::InspectionFailed);
    }
    Ok(ModuleBuildComponentInterface {
        exports: exports.into_iter().collect(),
        imports: imports.into_iter().collect(),
    })
}

fn component_name(
    name: wasmparser::ComponentExternName<'_>,
) -> Result<String, ComponentArtifactError> {
    if name.implements.is_some() || name.version_suffix.is_some() || name.external_id.is_some() {
        return Err(ComponentArtifactError::InspectionFailed);
    }
    valid_interface_name(name.name)
        .then(|| name.name.to_owned())
        .ok_or(ComponentArtifactError::InspectionFailed)
}

fn normalized_interface(
    interface: &ModuleBuildComponentInterface,
) -> Result<ModuleBuildComponentInterface, ComponentArtifactError> {
    let exports = normalize_names(&interface.exports)?;
    let imports = normalize_names(&interface.imports)?;
    if exports.is_empty() {
        return Err(ComponentArtifactError::InspectionFailed);
    }
    Ok(ModuleBuildComponentInterface { exports, imports })
}

fn normalize_interface_for_wit(
    interface: &ModuleBuildComponentInterface,
) -> Result<ModuleBuildComponentInterface, WitContractError> {
    let exports = normalize_names(&interface.exports).map_err(|_| WitContractError::Mismatch)?;
    let imports = normalize_names(&interface.imports).map_err(|_| WitContractError::Mismatch)?;
    if exports.is_empty() {
        return Err(WitContractError::Mismatch);
    }
    Ok(ModuleBuildComponentInterface { exports, imports })
}

fn normalize_names(values: &[String]) -> Result<Vec<String>, ComponentArtifactError> {
    let names = values.iter().collect::<BTreeSet<_>>();
    if names.len() != values.len() || names.iter().any(|name| !valid_interface_name(name)) {
        return Err(ComponentArtifactError::InspectionFailed);
    }
    Ok(names.into_iter().cloned().collect())
}

fn valid_interface_name(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_INTERFACE_NAME_BYTES
        && !value.contains(char::is_control)
}

fn valid_wit_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_INTERFACE_NAME_BYTES
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    fn digest(marker: char) -> String {
        format!("sha256:{}", marker.to_string().repeat(64))
    }

    fn provenance() -> serde_json::Value {
        json!({
            "_type": "https://in-toto.io/Statement/v1",
            "predicateType": "https://slsa.dev/provenance/v1",
            "subject": [{ "digest": { "sha256": "c".repeat(64) } }],
            "predicate": {
                "buildDefinition": {
                    "externalParameters": {
                        "rustok": {
                            "sourceDigest": digest('a'),
                            "dependencyLockDigest": digest('b'),
                            "toolchainDigest": digest('d'),
                            "witDigest": digest('e'),
                            "sdkVersion": "1.2.3",
                            "templateVersion": "4.5.6",
                            "expectedModuleSlug": "example",
                            "expectedVersion": "1.0.0",
                            "runtimeAbi": "wit-component-v1",
                            "attempt": 1,
                            "validationProfiles": [
                                "format",
                                "check",
                                "lint",
                                "test",
                                "dependency_policy",
                                "vulnerability"
                            ],
                            "validationResults": [
                                { "profile": "check", "outcome": "passed" },
                                { "profile": "test", "outcome": "passed" }
                            ]
                        }
                    }
                },
                "runDetails": { "builder": { "id": "rustok.module.build" } }
            }
        })
    }

    fn evidence_expectation() -> BuildEvidenceExpectation {
        BuildEvidenceExpectation {
            component_digest: digest('c'),
            sbom_digest: digest('f'),
            provenance_digest: digest('9'),
            source_digest: digest('a'),
            dependency_lock_digest: digest('b'),
            toolchain_digest: digest('d'),
            wit_digest: digest('e'),
            sdk_version: "1.2.3".to_string(),
            template_version: "4.5.6".to_string(),
            expected_module_slug: "example".to_string(),
            expected_version: "1.0.0".to_string(),
            runtime_abi: "wit-component-v1".to_string(),
            attempt: 1,
            validation_profiles: json!([
                "format",
                "check",
                "lint",
                "test",
                "dependency_policy",
                "vulnerability"
            ]),
            validation_results: json!([
                { "profile": "check", "outcome": "passed" },
                { "profile": "test", "outcome": "passed" }
            ]),
        }
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn create() -> Self {
            let path = std::env::temp_dir().join(format!(
                "rustok-module-build-artifact-test-{}",
                Uuid::new_v4()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn provenance_rejects_a_substituted_immutable_request_fact() {
        let document = provenance();
        let expectation = evidence_expectation();
        assert!(inspect_slsa_provenance(&document, &expectation).is_ok());

        let mut substituted = evidence_expectation();
        substituted.sdk_version = "1.2.4".to_string();
        assert!(matches!(
            inspect_slsa_provenance(&document, &substituted),
            Err(BuildEvidenceError::ProvenanceInvalid)
        ));
    }

    #[test]
    fn worker_finalizes_the_descriptor_once_with_the_verified_component_digest() {
        let source_bytes = serde_json::to_vec(&json!({
            "schema_version": rustok_modules::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            "slug": "example",
            "version": "1.0.0",
            "payload_kind": "wasm_component",
            "module_kind": "optional",
            "runtime_abi": rustok_modules::MODULE_BUILD_RUNTIME_ABI,
            "platform_compatibility": "^0.1",
            "entrypoint": "run"
        }))
        .expect("serialize source manifest");
        let source =
            ModuleArtifactSourceManifest::parse(&source_bytes).expect("parse source manifest");
        let output = TestDirectory::create();

        finalize_descriptor(source, &output.0, digest('c')).expect("finalize descriptor");
        let descriptor_bytes =
            fs::read(output.0.join(DESCRIPTOR_OUTPUT_FILE)).expect("read descriptor");
        let descriptor: ModuleArtifactDescriptor =
            serde_json::from_slice(&descriptor_bytes).expect("parse finalized descriptor");
        assert_eq!(descriptor.artifact_digest, digest('c'));
        assert!(descriptor.validate().is_ok());

        let duplicate = ModuleArtifactSourceManifest::parse(&source_bytes)
            .expect("parse duplicate source manifest");
        assert!(matches!(
            finalize_descriptor(duplicate, &output.0, digest('d')),
            Err(ArtifactDescriptorError::Invalid)
        ));
    }
}
