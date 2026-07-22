use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use rustok_modules::{ModuleBuildDependencyPolicy, ModuleBuildRequest};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    time::timeout,
};

const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_MANIFEST_COUNT: usize = 256;
const MAX_LOCK_PACKAGES: usize = 4_096;
const MAX_LOCK_DEPENDENCIES: usize = 32_768;
const MAX_LOCK_TEXT_BYTES: usize = 512;

/// The outcome of the worker-side source and dependency-policy preflight.
///
/// Policy denials are terminal facts about immutable source. I/O failures stay
/// distinct so the delivery path may retry a worker whose mounted source is
/// temporarily unavailable.
#[derive(Debug)]
pub enum SourcePolicyError {
    DependencyPolicyDenied,
    BuildScriptDenied,
    NativeLinkDenied,
    Internal(String),
}

/// Fail-closed result of an image-owned `cargo metadata --locked --offline`
/// invocation. No source-controlled executable or Cargo configuration chooses
/// this command or its cache directory.
#[derive(Debug)]
pub enum CargoMetadataError {
    DependencyPolicyDenied,
    BuildScriptDenied,
    NativeLinkDenied,
    ResourceLimit,
    NetworkPolicyDenied,
    Internal(String),
}

/// Fixed Cargo tool and verified offline dependency cache used exclusively for
/// metadata inspection. A scoped materializer may supply a fresh job-local
/// cache, but Cargo remains forced offline in this worker process.
pub struct CargoMetadataInspector {
    cargo_path: PathBuf,
    cargo_home: PathBuf,
}

impl CargoMetadataInspector {
    pub fn new(cargo_path: PathBuf, cargo_home: PathBuf) -> Result<Self, String> {
        validate_regular_file(&cargo_path, "module build Cargo path")?;
        validate_offline_cargo_home(&cargo_home)?;
        Ok(Self {
            cargo_path,
            cargo_home,
        })
    }

    pub async fn inspect(
        &self,
        source_dir: &Path,
        request: &ModuleBuildRequest,
        cargo_home: &Path,
        execution_timeout: Duration,
    ) -> Result<(), CargoMetadataError> {
        validate_offline_cargo_home(cargo_home).map_err(CargoMetadataError::Internal)?;
        if execution_timeout.is_zero() {
            return Err(CargoMetadataError::ResourceLimit);
        }
        let output_budget = Arc::new(MetadataOutputBudget::new(request.limits.output_bytes));
        let mut child = Command::new(&self.cargo_path)
            .current_dir(source_dir)
            .env_clear()
            .env("CARGO_HOME", cargo_home)
            .env("CARGO_NET_OFFLINE", "true")
            .env("CARGO_TERM_COLOR", "never")
            .env("RUSTUP_TOOLCHAIN", &request.toolchain.rust_toolchain)
            .args(["metadata", "--format-version", "1", "--locked", "--offline"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| CargoMetadataError::Internal(error.to_string()))?;
        let stdout = child.stdout.take().ok_or_else(|| {
            CargoMetadataError::Internal("Cargo metadata stdout is unavailable".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            CargoMetadataError::Internal("Cargo metadata stderr is unavailable".to_string())
        })?;
        let stdout_task = tokio::spawn(read_metadata_stream(
            stdout,
            Arc::clone(&output_budget),
            true,
        ));
        let stderr_task = tokio::spawn(read_metadata_stream(stderr, output_budget, false));
        let status = match timeout(execution_timeout, child.wait()).await {
            Ok(status) => {
                status.map_err(|error| CargoMetadataError::Internal(error.to_string()))?
            }
            Err(_) => {
                let _ = child.kill().await;
                stdout_task.abort();
                stderr_task.abort();
                return Err(CargoMetadataError::ResourceLimit);
            }
        };
        let stdout = collect_metadata_output(stdout_task).await?;
        collect_metadata_output(stderr_task).await?;
        if !status.success() {
            return Err(CargoMetadataError::DependencyPolicyDenied);
        }
        inspect_metadata_document(&stdout, source_dir, request)
    }

    pub fn default_cargo_home(&self) -> &Path {
        &self.cargo_home
    }

    pub fn cargo_path(&self) -> &Path {
        &self.cargo_path
    }
}

fn validate_regular_file(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute"));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{label} {} cannot be inspected: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{label} must be a regular non-symlink file"));
    }
    Ok(())
}

fn validate_directory(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute"));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{label} {} cannot be inspected: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{label} must be a directory, not a symlink"));
    }
    Ok(())
}

/// Cargo homes supplied to the offline build stages are dependency caches, not
/// a configuration or credential channel. Registry selection is already bound
/// by the immutable lock graph and scoped materializer receipt.
pub(crate) fn validate_offline_cargo_home(path: &Path) -> Result<(), String> {
    validate_directory(path, "module build Cargo home")?;
    for name in ["config", "config.toml", "credentials", "credentials.toml"] {
        match fs::symlink_metadata(path.join(name)) {
            Ok(_) => {
                return Err(format!("module build Cargo home must not contain {name}"));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "module build Cargo home {} cannot inspect {name}: {error}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

struct MetadataOutputBudget {
    limit: u64,
    consumed: AtomicU64,
}

impl MetadataOutputBudget {
    fn new(limit: u64) -> Self {
        Self {
            limit,
            consumed: AtomicU64::new(0),
        }
    }

    fn reserve(&self, bytes: usize) -> bool {
        let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
        let previous = self.consumed.fetch_add(bytes, Ordering::Relaxed);
        previous.saturating_add(bytes) <= self.limit
    }
}

async fn read_metadata_stream<R>(
    mut reader: R,
    budget: Arc<MetadataOutputBudget>,
    retain: bool,
) -> Result<Vec<u8>, CargoMetadataError>
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
            .map_err(|error| CargoMetadataError::Internal(error.to_string()))?;
        if read == 0 {
            return if exceeded {
                Err(CargoMetadataError::ResourceLimit)
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

async fn collect_metadata_output(
    task: tokio::task::JoinHandle<Result<Vec<u8>, CargoMetadataError>>,
) -> Result<Vec<u8>, CargoMetadataError> {
    task.await.map_err(|error| {
        CargoMetadataError::Internal(format!("Cargo metadata output reader failed: {error}"))
    })?
}

fn inspect_metadata_document(
    output: &[u8],
    source_dir: &Path,
    request: &ModuleBuildRequest,
) -> Result<(), CargoMetadataError> {
    let document: serde_json::Value =
        serde_json::from_slice(output).map_err(|_| CargoMetadataError::DependencyPolicyDenied)?;
    let packages = document
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .filter(|packages| !packages.is_empty() && packages.len() <= MAX_LOCK_PACKAGES)
        .ok_or(CargoMetadataError::DependencyPolicyDenied)?;
    let workspace_members = document
        .get("workspace_members")
        .and_then(serde_json::Value::as_array)
        .filter(|members| !members.is_empty())
        .ok_or(CargoMetadataError::DependencyPolicyDenied)?;
    let workspace_ids = workspace_members
        .iter()
        .map(|member| member.as_str().map(str::to_owned))
        .collect::<Option<std::collections::BTreeSet<_>>>()
        .ok_or(CargoMetadataError::DependencyPolicyDenied)?;
    let source_dir = fs::canonicalize(source_dir).map_err(|error| {
        CargoMetadataError::Internal(format!(
            "metadata source root cannot be canonicalized: {error}"
        ))
    })?;
    let mut package_ids = std::collections::BTreeSet::new();
    let mut dependency_count = 0_usize;
    for package in packages {
        let object = package
            .as_object()
            .ok_or(CargoMetadataError::DependencyPolicyDenied)?;
        let id = metadata_text(object, "id")?;
        let name = metadata_text(object, "name")?;
        let _version = metadata_text(object, "version")?;
        if !valid_package_name(name) || !package_ids.insert(id.to_owned()) {
            return Err(CargoMetadataError::DependencyPolicyDenied);
        }
        let source = object.get("source").and_then(serde_json::Value::as_str);
        if let Some(source) = source {
            inspect_metadata_source(source, request)?;
        } else if workspace_ids.contains(id) {
            let manifest_path = PathBuf::from(metadata_text(object, "manifest_path")?);
            let manifest_path = fs::canonicalize(&manifest_path).map_err(|error| {
                CargoMetadataError::Internal(format!(
                    "workspace manifest {} cannot be canonicalized: {error}",
                    manifest_path.display()
                ))
            })?;
            if !manifest_path.starts_with(&source_dir) {
                return Err(CargoMetadataError::DependencyPolicyDenied);
            }
        } else {
            return Err(CargoMetadataError::DependencyPolicyDenied);
        }
        if !request.dependency_policy.allow_native_links
            && object.get("links").is_some_and(|links| !links.is_null())
        {
            return Err(CargoMetadataError::NativeLinkDenied);
        }
        inspect_metadata_targets(object.get("targets"), request)?;
        let dependencies = object
            .get("dependencies")
            .and_then(serde_json::Value::as_array)
            .ok_or(CargoMetadataError::DependencyPolicyDenied)?;
        dependency_count = dependency_count
            .checked_add(dependencies.len())
            .ok_or(CargoMetadataError::ResourceLimit)?;
        if dependency_count > MAX_LOCK_DEPENDENCIES {
            return Err(CargoMetadataError::ResourceLimit);
        }
        for dependency in dependencies {
            inspect_metadata_dependency(dependency, request)?;
        }
    }
    inspect_metadata_resolve(document.get("resolve"), &package_ids)?;
    Ok(())
}

fn metadata_text<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, CargoMetadataError> {
    object
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= MAX_LOCK_TEXT_BYTES)
        .ok_or(CargoMetadataError::DependencyPolicyDenied)
}

fn inspect_metadata_source(
    source: &str,
    request: &ModuleBuildRequest,
) -> Result<(), CargoMetadataError> {
    if source.len() > MAX_LOCK_TEXT_BYTES {
        return Err(CargoMetadataError::DependencyPolicyDenied);
    }
    if let Some(registry) = source.strip_prefix("registry+") {
        return if registry_is_allowed(registry, &request.dependency_policy) {
            Ok(())
        } else {
            Err(CargoMetadataError::DependencyPolicyDenied)
        };
    }
    if let Some(git) = source.strip_prefix("git+") {
        return if request.dependency_policy.allow_git_dependencies
            && git_source_has_pinned_revision(git)
        {
            Ok(())
        } else {
            Err(CargoMetadataError::DependencyPolicyDenied)
        };
    }
    Err(CargoMetadataError::DependencyPolicyDenied)
}

fn inspect_metadata_targets(
    targets: Option<&serde_json::Value>,
    request: &ModuleBuildRequest,
) -> Result<(), CargoMetadataError> {
    let targets = targets
        .and_then(serde_json::Value::as_array)
        .ok_or(CargoMetadataError::DependencyPolicyDenied)?;
    for target in targets {
        let target = target
            .as_object()
            .ok_or(CargoMetadataError::DependencyPolicyDenied)?;
        let kinds = target
            .get("kind")
            .and_then(serde_json::Value::as_array)
            .ok_or(CargoMetadataError::DependencyPolicyDenied)?;
        if !request.dependency_policy.allow_build_scripts
            && kinds
                .iter()
                .any(|kind| kind.as_str() == Some("custom-build"))
        {
            return Err(CargoMetadataError::BuildScriptDenied);
        }
    }
    Ok(())
}

fn inspect_metadata_dependency(
    dependency: &serde_json::Value,
    request: &ModuleBuildRequest,
) -> Result<(), CargoMetadataError> {
    let dependency = dependency
        .as_object()
        .ok_or(CargoMetadataError::DependencyPolicyDenied)?;
    if dependency.get("path").is_some_and(|path| !path.is_null()) {
        return Err(CargoMetadataError::DependencyPolicyDenied);
    }
    if let Some(source) = dependency.get("source").and_then(serde_json::Value::as_str) {
        inspect_metadata_source(source, request)?;
    }
    Ok(())
}

fn inspect_metadata_resolve(
    resolve: Option<&serde_json::Value>,
    package_ids: &std::collections::BTreeSet<String>,
) -> Result<(), CargoMetadataError> {
    let nodes = resolve
        .and_then(serde_json::Value::as_object)
        .and_then(|resolve| resolve.get("nodes"))
        .and_then(serde_json::Value::as_array)
        .filter(|nodes| !nodes.is_empty() && nodes.len() <= package_ids.len())
        .ok_or(CargoMetadataError::DependencyPolicyDenied)?;
    let mut resolved_ids = std::collections::BTreeSet::new();
    for node in nodes {
        let node = node
            .as_object()
            .ok_or(CargoMetadataError::DependencyPolicyDenied)?;
        let id = metadata_text(node, "id")?;
        if !package_ids.contains(id) || !resolved_ids.insert(id.to_owned()) {
            return Err(CargoMetadataError::DependencyPolicyDenied);
        }
    }
    if &resolved_ids != package_ids {
        return Err(CargoMetadataError::DependencyPolicyDenied);
    }
    Ok(())
}

/// Fail-closed Cargo preflight performed before the fixed runner receives a
/// materialized source tree.
///
/// The request lock digest is the SHA-256 digest of the raw `Cargo.lock`
/// bytes. This deliberately binds the exact lock representation, including
/// source URLs and checksums, rather than a lossy reserialization. The fixed
/// runner must still perform its own locked graph inspection before executing
/// Cargo commands; this preflight prevents obvious policy bypasses at the
/// worker boundary.
pub struct SourcePolicyPreflight;

impl SourcePolicyPreflight {
    pub async fn inspect(
        source_dir: &Path,
        request: &ModuleBuildRequest,
    ) -> Result<(), SourcePolicyError> {
        let source_dir = source_dir.to_path_buf();
        let policy = request.dependency_policy.clone();
        let disk_limit = request.limits.disk_bytes;
        tokio::task::spawn_blocking(move || inspect_source(&source_dir, &policy, disk_limit))
            .await
            .map_err(|error| {
                SourcePolicyError::Internal(format!("policy preflight failed: {error}"))
            })?
    }
}

fn inspect_source(
    source_dir: &Path,
    policy: &ModuleBuildDependencyPolicy,
    disk_limit: u64,
) -> Result<(), SourcePolicyError> {
    let lock_path = source_dir.join("Cargo.lock");
    let lock_bytes = read_bounded(&lock_path, disk_limit)?;
    let actual_lock_digest = format!("sha256:{}", hex::encode(Sha256::digest(&lock_bytes)));
    if actual_lock_digest != policy.lock_digest {
        return Err(SourcePolicyError::DependencyPolicyDenied);
    }
    let lock = std::str::from_utf8(&lock_bytes)
        .map_err(|_| SourcePolicyError::DependencyPolicyDenied)?
        .parse::<toml::Value>()
        .map_err(|_| SourcePolicyError::DependencyPolicyDenied)?;
    inspect_resolved_lock_graph(&lock, policy)?;

    reject_cargo_config(source_dir)?;
    let manifests = find_manifests(source_dir)?;
    if manifests.is_empty() {
        return Err(SourcePolicyError::DependencyPolicyDenied);
    }
    for manifest_path in manifests {
        inspect_manifest(&manifest_path, policy)?;
    }
    Ok(())
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, SourcePolicyError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            SourcePolicyError::DependencyPolicyDenied
        } else {
            SourcePolicyError::Internal(format!("{} cannot be inspected: {error}", path.display()))
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > limit {
        return Err(SourcePolicyError::DependencyPolicyDenied);
    }
    fs::read(path).map_err(|error| {
        SourcePolicyError::Internal(format!("{} cannot be read: {error}", path.display()))
    })
}

fn reject_cargo_config(source_dir: &Path) -> Result<(), SourcePolicyError> {
    // Cargo discovers project configuration from the current directory through
    // every ancestor. Checking only the extracted source root would leave a
    // deployment-owned workdir configuration able to alter the fixed offline
    // command or its registry behavior.
    for directory in source_dir.ancestors() {
        let cargo_dir = directory.join(".cargo");
        for name in ["config", "config.toml"] {
            let path = cargo_dir.join(name);
            match fs::symlink_metadata(&path) {
                Ok(_) => return Err(SourcePolicyError::DependencyPolicyDenied),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(SourcePolicyError::Internal(format!(
                        "{} cannot be inspected: {error}",
                        path.display()
                    )));
                }
            }
        }
    }
    Ok(())
}

fn find_manifests(source_dir: &Path) -> Result<Vec<PathBuf>, SourcePolicyError> {
    let mut pending = vec![source_dir.to_path_buf()];
    let mut manifests = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory).map_err(|error| {
            SourcePolicyError::Internal(format!(
                "{} cannot be listed: {error}",
                directory.display()
            ))
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| SourcePolicyError::Internal(error.to_string()))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                SourcePolicyError::Internal(format!(
                    "{} cannot be inspected: {error}",
                    path.display()
                ))
            })?;
            if metadata.file_type().is_symlink() {
                return Err(SourcePolicyError::DependencyPolicyDenied);
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() && entry.file_name() == "Cargo.toml" {
                if metadata.len() > MAX_MANIFEST_BYTES || manifests.len() == MAX_MANIFEST_COUNT {
                    return Err(SourcePolicyError::DependencyPolicyDenied);
                }
                manifests.push(path);
            }
        }
    }
    Ok(manifests)
}

fn inspect_manifest(
    manifest_path: &Path,
    policy: &ModuleBuildDependencyPolicy,
) -> Result<(), SourcePolicyError> {
    let contents = read_bounded(manifest_path, MAX_MANIFEST_BYTES)?;
    let manifest = std::str::from_utf8(&contents)
        .map_err(|_| SourcePolicyError::DependencyPolicyDenied)?
        .parse::<toml::Value>()
        .map_err(|_| SourcePolicyError::DependencyPolicyDenied)?;
    let table = manifest
        .as_table()
        .ok_or(SourcePolicyError::DependencyPolicyDenied)?;
    if let Some(package) = table.get("package").and_then(toml::Value::as_table) {
        let build_is_disabled = matches!(package.get("build"), Some(toml::Value::Boolean(false)));
        if !policy.allow_build_scripts
            && (!build_is_disabled
                && (package.contains_key("build")
                    || manifest_path.with_file_name("build.rs").exists()))
        {
            return Err(SourcePolicyError::BuildScriptDenied);
        }
        if !policy.allow_native_links && package.contains_key("links") {
            return Err(SourcePolicyError::NativeLinkDenied);
        }
    }
    inspect_manifest_table(table, policy)
}

fn inspect_manifest_table(
    table: &toml::map::Map<String, toml::Value>,
    policy: &ModuleBuildDependencyPolicy,
) -> Result<(), SourcePolicyError> {
    for (key, value) in table {
        match key.as_str() {
            "patch" | "replace" => return Err(SourcePolicyError::DependencyPolicyDenied),
            "dependencies" | "dev-dependencies" | "build-dependencies" => {
                inspect_dependency_value(value, policy)?;
            }
            _ => inspect_manifest_value(value, policy)?,
        }
    }
    Ok(())
}

fn inspect_manifest_value(
    value: &toml::Value,
    policy: &ModuleBuildDependencyPolicy,
) -> Result<(), SourcePolicyError> {
    match value {
        toml::Value::Table(table) => inspect_manifest_table(table, policy),
        toml::Value::Array(values) => {
            for value in values {
                inspect_manifest_value(value, policy)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn inspect_dependency_value(
    value: &toml::Value,
    policy: &ModuleBuildDependencyPolicy,
) -> Result<(), SourcePolicyError> {
    match value {
        toml::Value::Table(table) => {
            for (key, value) in table {
                match key.as_str() {
                    "git" if !policy.allow_git_dependencies => {
                        return Err(SourcePolicyError::DependencyPolicyDenied);
                    }
                    "path" => return Err(SourcePolicyError::DependencyPolicyDenied),
                    "registry" | "registry-index" => {
                        let registry = value
                            .as_str()
                            .ok_or(SourcePolicyError::DependencyPolicyDenied)?;
                        if !registry_is_allowed(registry, policy) {
                            return Err(SourcePolicyError::DependencyPolicyDenied);
                        }
                    }
                    _ => inspect_dependency_value(value, policy)?,
                }
            }
            Ok(())
        }
        toml::Value::Array(values) => {
            for value in values {
                inspect_dependency_value(value, policy)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Inspect the graph Cargo has already resolved and persisted. Running
/// `cargo metadata` remains a later, runner-owned step after controlled
/// dependency materialization; this parser performs no network or code
/// execution before the worker accepts immutable source.
fn inspect_resolved_lock_graph(
    lock: &toml::Value,
    policy: &ModuleBuildDependencyPolicy,
) -> Result<(), SourcePolicyError> {
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or(SourcePolicyError::DependencyPolicyDenied)?;
    if packages.is_empty() || packages.len() > MAX_LOCK_PACKAGES {
        return Err(SourcePolicyError::DependencyPolicyDenied);
    }

    let mut package_names = std::collections::BTreeSet::new();
    let mut package_ids = std::collections::BTreeSet::new();
    let mut total_dependencies = 0_usize;
    for package in packages {
        let table = package
            .as_table()
            .ok_or(SourcePolicyError::DependencyPolicyDenied)?;
        let name = lock_text(table, "name")?;
        let version = lock_text(table, "version")?;
        if !valid_package_name(name) {
            return Err(SourcePolicyError::DependencyPolicyDenied);
        }
        package_names.insert(name.to_owned());
        let source = package.get("source").and_then(toml::Value::as_str);
        let package_id = format!("{name}\0{version}\0{}", source.unwrap_or_default());
        if !package_ids.insert(package_id) {
            return Err(SourcePolicyError::DependencyPolicyDenied);
        }
        let Some(source) = source else {
            if table.contains_key("checksum") {
                return Err(SourcePolicyError::DependencyPolicyDenied);
            }
            inspect_locked_dependencies(table, &mut total_dependencies)?;
            continue;
        };
        if source.len() > MAX_LOCK_TEXT_BYTES {
            return Err(SourcePolicyError::DependencyPolicyDenied);
        }
        inspect_locked_source(source, table.get("checksum"), policy)?;
        inspect_locked_dependencies(table, &mut total_dependencies)?;
    }
    if total_dependencies > MAX_LOCK_DEPENDENCIES {
        return Err(SourcePolicyError::DependencyPolicyDenied);
    }
    for package in packages {
        let table = package
            .as_table()
            .ok_or(SourcePolicyError::DependencyPolicyDenied)?;
        let Some(dependencies) = table.get("dependencies") else {
            continue;
        };
        for dependency in dependencies
            .as_array()
            .ok_or(SourcePolicyError::DependencyPolicyDenied)?
        {
            let dependency = dependency
                .as_str()
                .ok_or(SourcePolicyError::DependencyPolicyDenied)?;
            let dependency_name = dependency
                .split_ascii_whitespace()
                .next()
                .filter(|value| valid_package_name(value))
                .ok_or(SourcePolicyError::DependencyPolicyDenied)?;
            if !package_names.contains(dependency_name) {
                return Err(SourcePolicyError::DependencyPolicyDenied);
            }
        }
    }
    Ok(())
}

fn lock_text<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<&'a str, SourcePolicyError> {
    let value = table
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= MAX_LOCK_TEXT_BYTES)
        .ok_or(SourcePolicyError::DependencyPolicyDenied)?;
    Ok(value)
}

fn inspect_locked_dependencies(
    package: &toml::map::Map<String, toml::Value>,
    total_dependencies: &mut usize,
) -> Result<(), SourcePolicyError> {
    let Some(dependencies) = package.get("dependencies") else {
        return Ok(());
    };
    let dependencies = dependencies
        .as_array()
        .ok_or(SourcePolicyError::DependencyPolicyDenied)?;
    *total_dependencies = total_dependencies
        .checked_add(dependencies.len())
        .ok_or(SourcePolicyError::DependencyPolicyDenied)?;
    Ok(())
}

fn inspect_locked_source(
    source: &str,
    checksum: Option<&toml::Value>,
    policy: &ModuleBuildDependencyPolicy,
) -> Result<(), SourcePolicyError> {
    if let Some(registry) = source.strip_prefix("registry+") {
        if !registry_is_allowed(registry, policy) || !valid_registry_checksum(checksum) {
            return Err(SourcePolicyError::DependencyPolicyDenied);
        }
        return Ok(());
    }
    if let Some(git) = source.strip_prefix("git+") {
        if !policy.allow_git_dependencies
            || checksum.is_some()
            || !git_source_has_pinned_revision(git)
        {
            return Err(SourcePolicyError::DependencyPolicyDenied);
        }
        return Ok(());
    }
    Err(SourcePolicyError::DependencyPolicyDenied)
}

fn valid_registry_checksum(checksum: Option<&toml::Value>) -> bool {
    checksum
        .and_then(toml::Value::as_str)
        .is_some_and(|checksum| {
            checksum.len() == 64
                && checksum
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
        })
}

fn git_source_has_pinned_revision(source: &str) -> bool {
    let Some((repository, revision)) = source.rsplit_once('#') else {
        return false;
    };
    !repository.is_empty()
        && !url_has_credentials(repository)
        && (40..=128).contains(&revision.len())
        && revision
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn valid_package_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
}

fn registry_is_allowed(registry: &str, policy: &ModuleBuildDependencyPolicy) -> bool {
    !url_has_credentials(registry)
        && policy.allowed_registries.iter().any(|allowed| {
            !url_has_credentials(allowed)
                && (allowed == registry
                    || (allowed == "https://crates.io"
                        && registry == "https://github.com/rust-lang/crates.io-index"))
        })
}

fn url_has_credentials(value: &str) -> bool {
    value
        .split_once("://")
        .is_some_and(|(_, authority_and_path)| {
            authority_and_path
                .split('/')
                .next()
                .is_some_and(|authority| authority.contains('@'))
        })
}

#[cfg(test)]
mod tests {
    use super::{SourcePolicyError, inspect_resolved_lock_graph};
    use rustok_modules::ModuleBuildDependencyPolicy;

    fn policy() -> ModuleBuildDependencyPolicy {
        ModuleBuildDependencyPolicy {
            lock_digest: format!("sha256:{}", "a".repeat(64)),
            allowed_registries: vec!["https://crates.io".to_string()],
            allow_git_dependencies: false,
            allow_build_scripts: false,
            allow_native_links: false,
        }
    }

    fn lock(source: &str) -> toml::Value {
        source.parse().expect("Cargo.lock fixture")
    }

    fn package_mut(lock: &mut toml::Value, index: usize) -> &mut toml::Value {
        lock.get_mut("package")
            .and_then(toml::Value::as_array_mut)
            .and_then(|packages| packages.get_mut(index))
            .expect("package fixture")
    }

    #[test]
    fn resolved_lock_fixture_accepts_only_allowlisted_checksummed_registry_graph() {
        let valid = lock(&format!(
            r#"
[[package]]
name = "module"
version = "1.0.0"
dependencies = ["dependency 1.2.3"]

[[package]]
name = "dependency"
version = "1.2.3"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "{}"
"#,
            "b".repeat(64)
        ));

        assert!(inspect_resolved_lock_graph(&valid, &policy()).is_ok());

        let mut missing_checksum = valid.clone();
        package_mut(&mut missing_checksum, 1)
            .as_table_mut()
            .expect("dependency package")
            .remove("checksum");
        assert!(matches!(
            inspect_resolved_lock_graph(&missing_checksum, &policy()),
            Err(SourcePolicyError::DependencyPolicyDenied)
        ));

        let mut credential_source = valid.clone();
        package_mut(&mut credential_source, 1)
            .as_table_mut()
            .expect("dependency package")
            .insert(
                "source".to_string(),
                toml::Value::String("registry+https://token@crates.io/index".to_string()),
            );
        assert!(matches!(
            inspect_resolved_lock_graph(&credential_source, &policy()),
            Err(SourcePolicyError::DependencyPolicyDenied)
        ));
    }

    #[test]
    fn resolved_lock_fixture_rejects_unpinned_git_and_dangling_dependencies() {
        let git = lock(
            r#"
[[package]]
name = "module"
version = "1.0.0"

[[package]]
name = "dependency"
version = "1.2.3"
source = "git+https://github.com/example/dependency"
"#,
        );
        let mut git_policy = policy();
        git_policy.allow_git_dependencies = true;
        assert!(matches!(
            inspect_resolved_lock_graph(&git, &git_policy),
            Err(SourcePolicyError::DependencyPolicyDenied)
        ));

        let dangling = lock(
            r#"
[[package]]
name = "module"
version = "1.0.0"
dependencies = ["missing 9.9.9"]
"#,
        );
        assert!(matches!(
            inspect_resolved_lock_graph(&dangling, &policy()),
            Err(SourcePolicyError::DependencyPolicyDenied)
        ));
    }

    #[test]
    fn resolved_lock_fixture_accepts_allowlisted_git_only_with_exact_revision() {
        let revision = "c".repeat(40);
        let git = lock(&format!(
            r#"
[[package]]
name = "dependency"
version = "1.2.3"
source = "git+https://github.com/example/dependency#{revision}"
"#
        ));
        let mut git_policy = policy();
        git_policy.allow_git_dependencies = true;

        assert!(inspect_resolved_lock_graph(&git, &git_policy).is_ok());
    }
}
