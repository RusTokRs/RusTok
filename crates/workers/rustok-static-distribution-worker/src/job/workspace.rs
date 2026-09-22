use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use rustok_build_source::{ArchiveLimits, CasArchiveError, CasArchiveReceipt, CasArchiveStore};
use rustok_distribution::generate_static_distribution;

use super::{
    PreparedStaticDistributionWorkspace, StaticDistributionJobConfig, StaticDistributionJobError,
    StaticDistributionJobRequest, create_directory_path, digest_bytes, io_error,
    remove_owned_workspace, validate_directory, write_new_file,
};

const MAX_CARGO_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;
const MAX_CARGO_LOCK_BYTES: u64 = 32 * 1024 * 1024;

fn create_source_store_and_limits(
    config: &StaticDistributionJobConfig,
) -> Result<(CasArchiveStore, ArchiveLimits), StaticDistributionJobError> {
    let source_store = CasArchiveStore::new(config.cas_root.clone())?;
    let limits = ArchiveLimits::new(
        config.max_archive_bytes,
        config.max_source_extracted_bytes,
        config.max_archive_entries,
    )?;
    Ok((source_store, limits))
}

pub fn materialize_static_distribution_workspace(
    job_dir: &Path,
    request: &StaticDistributionJobRequest,
    generated_manifest_bytes: &[u8],
    cargo_dependencies_bytes: &[u8],
    registry_source_bytes: &[u8],
    config: &StaticDistributionJobConfig,
) -> Result<PreparedStaticDistributionWorkspace, StaticDistributionJobError> {
    let payloads = GeneratedDistributionPayloads {
        manifest_bytes: generated_manifest_bytes,
        cargo_dependencies_bytes,
        registry_source_bytes,
    };
    let generated = validate_materialization_inputs(job_dir, request, config, &payloads)?;
    let job_dir = fs::canonicalize(job_dir).map_err(io_error)?;
    let workspace = job_dir.join("workspace");
    let (source_store, limits) = create_source_store_and_limits(config)?;
    let result = materialize_sources_and_apply(
        &source_store,
        &workspace,
        request,
        &generated,
        &payloads,
        config,
        limits,
    );
    if result.is_err() {
        remove_owned_workspace(&job_dir, &workspace);
    }
    result
}

struct GeneratedDistributionPayloads<'a> {
    manifest_bytes: &'a [u8],
    cargo_dependencies_bytes: &'a [u8],
    registry_source_bytes: &'a [u8],
}

fn validate_materialization_inputs(
    job_dir: &Path,
    request: &StaticDistributionJobRequest,
    config: &StaticDistributionJobConfig,
    payloads: &GeneratedDistributionPayloads<'_>,
) -> Result<rustok_distribution::GeneratedStaticDistributionFiles, StaticDistributionJobError> {
    config.validate_runtime()?;
    request.work_item.validate().map_err(|error| {
        StaticDistributionJobError::InvalidInput(format!("work item is invalid: {error}"))
    })?;
    if request.toolchain_digest != config.toolchain_digest
        || request.build_target != config.build_target
    {
        return Err(StaticDistributionJobError::InvalidInput(
            "request does not match the job-config toolchain and target".to_string(),
        ));
    }
    validate_directory(job_dir, "job directory")?;
    let generated = generate_static_distribution(&request.work_item).map_err(|error| {
        StaticDistributionJobError::InvalidInput(format!(
            "generated distribution is invalid: {error}"
        ))
    })?;
    if generated.manifest.output_digest != request.generated_output_digest
        || generated.manifest_json != payloads.manifest_bytes
        || generated.cargo_dependencies_toml.as_bytes() != payloads.cargo_dependencies_bytes
        || generated.registry_source.as_bytes() != payloads.registry_source_bytes
    {
        return Err(StaticDistributionJobError::InvalidInput(
            "generated files do not match the immutable request".to_string(),
        ));
    }
    Ok(generated)
}

fn materialize_platform_source(
    source_store: &CasArchiveStore,
    workspace: &Path,
    request: &StaticDistributionJobRequest,
    config: &StaticDistributionJobConfig,
    limits: ArchiveLimits,
) -> Result<CasArchiveReceipt, StaticDistributionJobError> {
    let platform_source = source_store.materialize(
        &request.work_item.build.platform_source_reference,
        &request.work_item.build.platform_source_digest,
        workspace,
        limits,
    )?;
    if platform_source.extracted_bytes > config.max_total_extracted_bytes {
        return Err(StaticDistributionJobError::Source(
            CasArchiveError::ResourceLimit,
        ));
    }
    Ok(platform_source)
}

fn apply_workspace_modifications(
    workspace: &Path,
    generated: &rustok_distribution::GeneratedStaticDistributionFiles,
    payloads: &GeneratedDistributionPayloads<'_>,
) -> Result<(), StaticDistributionJobError> {
    apply_cargo_dependencies(
        workspace,
        &generated.manifest.cargo_manifest_path,
        payloads.cargo_dependencies_bytes,
    )?;
    replace_generated_file(
        workspace,
        &generated.manifest.registry_source_path,
        payloads.registry_source_bytes,
        false,
    )?;
    replace_generated_file(
        workspace,
        &generated.manifest.manifest_path,
        payloads.manifest_bytes,
        true,
    )
}

fn materialize_sources_and_apply(
    source_store: &CasArchiveStore,
    workspace: &Path,
    request: &StaticDistributionJobRequest,
    generated: &rustok_distribution::GeneratedStaticDistributionFiles,
    payloads: &GeneratedDistributionPayloads<'_>,
    config: &StaticDistributionJobConfig,
    limits: ArchiveLimits,
) -> Result<PreparedStaticDistributionWorkspace, StaticDistributionJobError> {
    let platform_source =
        materialize_platform_source(source_store, workspace, request, config, limits)?;
    let source_parent = workspace.join(".rustok").join("static-sources");
    create_directory_path(&source_parent)?;
    let (promoted_sources, total_extracted_bytes) = materialize_promoted_sources(
        source_store,
        workspace,
        &source_parent,
        &generated.manifest.sources,
        limits,
        config.max_total_extracted_bytes,
        platform_source.extracted_bytes,
    )?;
    apply_workspace_modifications(workspace, generated, payloads)?;
    Ok(PreparedStaticDistributionWorkspace {
        workspace: workspace.to_path_buf(),
        platform_source,
        promoted_sources,
        total_extracted_bytes,
    })
}

fn materialize_promoted_sources(
    source_store: &CasArchiveStore,
    workspace: &Path,
    source_parent: &Path,
    sources: &[rustok_distribution::GeneratedStaticDistributionSource],
    limits: ArchiveLimits,
    max_total_extracted_bytes: u64,
    mut total_extracted_bytes: u64,
) -> Result<(Vec<CasArchiveReceipt>, u64), StaticDistributionJobError> {
    let mut promoted_sources = Vec::with_capacity(sources.len());
    for source in sources {
        let relative = validated_relative_path(&source.materialization_path)?;
        let destination = workspace.join(relative);
        if destination.parent() != Some(source_parent) {
            return Err(StaticDistributionJobError::InvalidInput(
                "generated source path escaped the fixed materialization root".to_string(),
            ));
        }
        let receipt = source_store.materialize(
            &source.source_reference,
            &source.source_digest,
            &destination,
            limits,
        )?;
        total_extracted_bytes = total_extracted_bytes
            .checked_add(receipt.extracted_bytes)
            .ok_or(CasArchiveError::ResourceLimit)?;
        if total_extracted_bytes > max_total_extracted_bytes {
            return Err(StaticDistributionJobError::Source(
                CasArchiveError::ResourceLimit,
            ));
        }
        validate_promoted_package(&destination, source)?;
        promoted_sources.push(receipt);
    }
    Ok((promoted_sources, total_extracted_bytes))
}

fn validate_promoted_package(
    source_root: &Path,
    source: &rustok_distribution::GeneratedStaticDistributionSource,
) -> Result<(), StaticDistributionJobError> {
    let manifest_path = source_root.join("Cargo.toml");
    let manifest_bytes = read_bounded_regular(&manifest_path, MAX_CARGO_MANIFEST_BYTES)?;
    let manifest_text = std::str::from_utf8(&manifest_bytes).map_err(|_| {
        StaticDistributionJobError::InvalidInput("promoted Cargo manifest is not UTF-8".to_string())
    })?;
    let manifest = manifest_text.parse::<toml::Table>().map_err(|error| {
        StaticDistributionJobError::InvalidInput(format!(
            "promoted Cargo manifest is invalid: {error}"
        ))
    })?;
    let package = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            StaticDistributionJobError::InvalidInput(
                "promoted Cargo package table is missing".to_string(),
            )
        })?;
    if package.get("name").and_then(toml::Value::as_str) != Some(source.cargo_package.as_str())
        || package.get("version").and_then(toml::Value::as_str)
            != Some(source.module_version.as_str())
    {
        return Err(StaticDistributionJobError::InvalidInput(
            "promoted Cargo package identity does not match the reviewed release".to_string(),
        ));
    }
    let lock_path = source_root.join("Cargo.lock");
    let lock_bytes = read_bounded_regular(&lock_path, MAX_CARGO_LOCK_BYTES)?;
    if digest_bytes(&lock_bytes) != source.dependency_lock_digest {
        return Err(StaticDistributionJobError::InvalidInput(
            "promoted Cargo.lock does not match the reviewed dependency graph".to_string(),
        ));
    }
    Ok(())
}

fn apply_cargo_dependencies(
    workspace: &Path,
    relative_manifest_path: &str,
    cargo_dependencies_bytes: &[u8],
) -> Result<(), StaticDistributionJobError> {
    let relative = validated_relative_path(relative_manifest_path)?;
    let manifest_path = workspace.join(relative);
    let manifest_bytes = read_bounded_regular(&manifest_path, MAX_CARGO_MANIFEST_BYTES)?;
    let manifest_text = std::str::from_utf8(&manifest_bytes).map_err(|_| {
        StaticDistributionJobError::InvalidInput(
            "distribution Cargo manifest is not UTF-8".to_string(),
        )
    })?;
    let mut manifest = manifest_text.parse::<toml::Table>().map_err(|error| {
        StaticDistributionJobError::InvalidInput(format!(
            "distribution Cargo manifest is invalid: {error}"
        ))
    })?;
    let generated_dependencies = parse_dependencies_fragment(cargo_dependencies_bytes)?;
    insert_cargo_dependencies(&mut manifest, generated_dependencies)?;
    let output = toml::to_string_pretty(&manifest).map_err(|error| {
        StaticDistributionJobError::InvalidInput(format!(
            "distribution Cargo manifest could not be serialized: {error}"
        ))
    })?;
    overwrite_regular_file(&manifest_path, output.as_bytes())
}

fn parse_dependencies_fragment(
    cargo_dependencies_bytes: &[u8],
) -> Result<toml::Table, StaticDistributionJobError> {
    let fragment_text = std::str::from_utf8(cargo_dependencies_bytes).map_err(|_| {
        StaticDistributionJobError::InvalidInput(
            "generated Cargo dependency fragment is not UTF-8".to_string(),
        )
    })?;
    let fragment = format!("[dependencies]\n{fragment_text}")
        .parse::<toml::Table>()
        .map_err(|error| {
            StaticDistributionJobError::InvalidInput(format!(
                "generated Cargo dependency fragment is invalid: {error}"
            ))
        })?;
    fragment
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .cloned()
        .ok_or_else(|| {
            StaticDistributionJobError::InvalidInput(
                "generated Cargo dependencies are missing".to_string(),
            )
        })
}

fn insert_cargo_dependencies(
    manifest: &mut toml::Table,
    generated_dependencies: toml::Table,
) -> Result<(), StaticDistributionJobError> {
    let dependencies = manifest
        .entry("dependencies")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| {
            StaticDistributionJobError::InvalidInput(
                "distribution dependencies are not a table".to_string(),
            )
        })?;
    for (alias, dependency) in generated_dependencies {
        if dependencies.contains_key(&alias) {
            return Err(StaticDistributionJobError::InvalidInput(format!(
                "generated dependency alias already exists: {alias}"
            )));
        }
        dependencies.insert(alias, dependency);
    }
    Ok(())
}

fn replace_generated_file(
    workspace: &Path,
    relative_path: &str,
    bytes: &[u8],
    create_parent: bool,
) -> Result<(), StaticDistributionJobError> {
    let relative = validated_relative_path(relative_path)?;
    let path = workspace.join(relative);
    let parent = path.parent().ok_or_else(|| {
        StaticDistributionJobError::InvalidInput("generated output path has no parent".to_string())
    })?;
    if create_parent {
        create_directory_path(parent)?;
    } else {
        validate_directory(parent, "generated output parent")?;
    }
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(StaticDistributionJobError::InvalidInput(
                "generated output target is not a regular file".to_string(),
            ))
        }
        Ok(_) => overwrite_regular_file(&path, bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && create_parent => {
            write_new_file(&path, bytes)
        }
        Err(error) => Err(io_error(error)),
    }
}

fn validated_relative_path(value: &str) -> Result<PathBuf, StaticDistributionJobError> {
    let path = PathBuf::from(value);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(StaticDistributionJobError::InvalidInput(
            "generated output path is unsafe".to_string(),
        ));
    }
    Ok(path)
}



fn read_bounded_regular(
    path: &Path,
    max_bytes: u64,
) -> Result<Vec<u8>, StaticDistributionJobError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > max_bytes {
        return Err(StaticDistributionJobError::InvalidInput(
            "job file is not a bounded regular file".to_string(),
        ));
    }
    let mut file = fs::File::open(path).map_err(io_error)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(io_error)?;
    Ok(bytes)
}

fn overwrite_regular_file(path: &Path, bytes: &[u8]) -> Result<(), StaticDistributionJobError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(StaticDistributionJobError::InvalidInput(
            "workspace output target is not a regular file".to_string(),
        ));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(io_error)?;
    file.write_all(bytes).map_err(io_error)?;
    file.sync_all().map_err(io_error)
}
