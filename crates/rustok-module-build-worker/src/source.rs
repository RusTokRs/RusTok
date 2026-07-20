use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use rustok_modules::{ModuleBuildProtocolError, ModuleBuildRequest};
use sha2::{Digest, Sha256};

/// Deployment-mounted read-only source archive materializer.
///
/// A request may identify source only as `cas://sha256/<digest>`. The matching
/// `<hex>.tar` blob must be present under the image-mounted source root. This
/// module never receives a storage client or a request-selected filesystem path.
pub struct SourceMaterializer {
    source_root: PathBuf,
    workspace_root: PathBuf,
}

/// Source materialization outcome that the worker can map to a terminal build
/// result without pretending an immutable source-policy violation is a
/// retryable transport outage.
#[derive(Debug)]
pub enum SourceMaterializationError {
    DigestMismatch,
    UnsafeArchive,
    ResourceLimit,
    Internal(String),
}

impl SourceMaterializer {
    pub fn new(source_root: PathBuf, workspace_root: PathBuf) -> Result<Self, String> {
        let source_root = canonical_directory(source_root, "module build source root")?;
        let workspace_root = canonical_directory(workspace_root, "module build workdir")?;
        Ok(Self {
            source_root,
            workspace_root,
        })
    }

    pub async fn materialize(
        &self,
        request: &ModuleBuildRequest,
    ) -> Result<MaterializedSource, SourceMaterializationError> {
        let source_root = self.source_root.clone();
        let workspace_root = self.workspace_root.clone();
        let request_id = request.request_id;
        let expected_digest = request.source.digest.clone();
        let source_reference = request.source.reference.clone();
        let disk_limit = request.limits.disk_bytes;
        tokio::task::spawn_blocking(move || {
            materialize_archive(
                source_root,
                workspace_root,
                request_id,
                &source_reference,
                &expected_digest,
                disk_limit,
            )
        })
        .await
        .map_err(|error| {
            SourceMaterializationError::Internal(format!("source materializer failed: {error}"))
        })?
        .map_err(classify_materialization_error)
    }
}

fn classify_materialization_error(error: ModuleBuildProtocolError) -> SourceMaterializationError {
    match error {
        ModuleBuildProtocolError::InvalidLimits => SourceMaterializationError::ResourceLimit,
        ModuleBuildProtocolError::InvalidRequest
        | ModuleBuildProtocolError::InvalidDigest
        | ModuleBuildProtocolError::InvalidReference => SourceMaterializationError::UnsafeArchive,
        ModuleBuildProtocolError::Transport(message)
            if message.contains("archive digest does not match") =>
        {
            SourceMaterializationError::DigestMismatch
        }
        error => SourceMaterializationError::Internal(error.to_string()),
    }
}

/// Job-scoped source directory. Dropping it removes all materialized source so
/// a worker process does not retain an untrusted workspace between requests.
pub struct MaterializedSource {
    job_dir: PathBuf,
    source_dir: PathBuf,
}

impl MaterializedSource {
    pub fn source_dir(&self) -> &Path {
        &self.source_dir
    }

    pub fn job_dir(&self) -> &Path {
        &self.job_dir
    }
}

impl Drop for MaterializedSource {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.job_dir);
    }
}

fn canonical_directory(path: PathBuf, label: &str) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute"));
    }
    let metadata = fs::metadata(&path)
        .map_err(|error| format!("{label} {} cannot be inspected: {error}", path.display()))?;
    if !metadata.is_dir() {
        return Err(format!("{label} {} must be a directory", path.display()));
    }
    fs::canonicalize(&path).map_err(|error| {
        format!(
            "{label} {} cannot be canonicalized: {error}",
            path.display()
        )
    })
}

fn materialize_archive(
    source_root: PathBuf,
    workspace_root: PathBuf,
    request_id: uuid::Uuid,
    source_reference: &str,
    expected_digest: &str,
    disk_limit: u64,
) -> Result<MaterializedSource, ModuleBuildProtocolError> {
    if source_reference != format!("cas://{expected_digest}") {
        return Err(ModuleBuildProtocolError::InvalidRequest);
    }
    let digest_hex = expected_digest
        .strip_prefix("sha256:")
        .ok_or(ModuleBuildProtocolError::InvalidDigest)?;
    if digest_hex.len() != 64
        || !digest_hex
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, 'a'..='f'))
    {
        return Err(ModuleBuildProtocolError::InvalidDigest);
    }
    let archive_path = source_root.join(format!("{digest_hex}.tar"));
    let archive_metadata = fs::symlink_metadata(&archive_path).map_err(|error| {
        ModuleBuildProtocolError::Transport(format!(
            "immutable source archive {} is unavailable: {error}",
            archive_path.display()
        ))
    })?;
    if archive_metadata.file_type().is_symlink() || !archive_metadata.is_file() {
        return Err(ModuleBuildProtocolError::InvalidRequest);
    }
    if archive_metadata.len() > disk_limit {
        return Err(ModuleBuildProtocolError::InvalidLimits);
    }
    verify_archive_digest(&archive_path, expected_digest)?;

    let job_dir = workspace_root.join(request_id.to_string());
    if job_dir.exists() {
        let metadata = fs::symlink_metadata(&job_dir).map_err(|error| {
            ModuleBuildProtocolError::Transport(format!(
                "materialized job directory {} cannot be inspected: {error}",
                job_dir.display()
            ))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ModuleBuildProtocolError::InvalidRequest);
        }
        fs::remove_dir_all(&job_dir).map_err(|error| {
            ModuleBuildProtocolError::Transport(format!(
                "stale materialized job directory {} cannot be removed: {error}",
                job_dir.display()
            ))
        })?;
    }
    let source_dir = job_dir.join("source");
    fs::create_dir_all(&source_dir).map_err(|error| {
        ModuleBuildProtocolError::Transport(format!(
            "materialized source directory {} cannot be created: {error}",
            source_dir.display()
        ))
    })?;

    let result = extract_safe_archive(&archive_path, &source_dir, disk_limit);
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&job_dir);
        return Err(error);
    }
    Ok(MaterializedSource {
        job_dir,
        source_dir,
    })
}

fn verify_archive_digest(
    archive_path: &Path,
    expected_digest: &str,
) -> Result<(), ModuleBuildProtocolError> {
    let mut archive = File::open(archive_path)
        .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = archive
            .read(&mut buffer)
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("sha256:{}", hex::encode(hasher.finalize()));
    if actual != expected_digest {
        return Err(ModuleBuildProtocolError::Transport(
            "materialized source archive digest does not match the immutable request".to_string(),
        ));
    }
    Ok(())
}

fn extract_safe_archive(
    archive_path: &Path,
    source_dir: &Path,
    disk_limit: u64,
) -> Result<(), ModuleBuildProtocolError> {
    let mut archive = File::open(archive_path)
        .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
    let mut extracted_bytes = 0_u64;
    loop {
        let mut header = [0_u8; 512];
        archive
            .read_exact(&mut header)
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        if header.iter().all(|byte| *byte == 0) {
            return Ok(());
        }
        if &header[257..263] != b"ustar\0" {
            return Err(ModuleBuildProtocolError::InvalidRequest);
        }
        validate_ustar_checksum(&header)?;
        let relative_path = ustar_path(&header)?;
        if !safe_relative_path(&relative_path) {
            return Err(ModuleBuildProtocolError::InvalidRequest);
        }
        let destination = source_dir.join(&relative_path);
        if !destination.starts_with(source_dir) {
            return Err(ModuleBuildProtocolError::InvalidRequest);
        }
        let entry_type = header[156];
        let entry_size = parse_octal(&header[124..136])?;
        if entry_type == b'5' {
            if entry_size != 0 {
                return Err(ModuleBuildProtocolError::InvalidRequest);
            }
            fs::create_dir_all(&destination)
                .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
            continue;
        }
        if entry_type != 0 && entry_type != b'0' {
            return Err(ModuleBuildProtocolError::InvalidRequest);
        }
        extracted_bytes = extracted_bytes
            .checked_add(entry_size)
            .ok_or(ModuleBuildProtocolError::InvalidLimits)?;
        if extracted_bytes > disk_limit {
            return Err(ModuleBuildProtocolError::InvalidLimits);
        }
        let parent = destination
            .parent()
            .ok_or(ModuleBuildProtocolError::InvalidRequest)?;
        fs::create_dir_all(parent)
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        let mut destination_file = File::create(&destination)
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        copy_exact_archive_bytes(&mut archive, &mut destination_file, entry_size)?;
        discard_archive_padding(&mut archive, entry_size)?;
    }
}

fn validate_ustar_checksum(header: &[u8; 512]) -> Result<(), ModuleBuildProtocolError> {
    let expected = parse_octal(&header[148..156])?;
    let actual = header
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            if (148..156).contains(&index) {
                u64::from(b' ')
            } else {
                u64::from(*byte)
            }
        })
        .sum::<u64>();
    if expected != actual {
        return Err(ModuleBuildProtocolError::InvalidRequest);
    }
    Ok(())
}

fn ustar_path(header: &[u8; 512]) -> Result<PathBuf, ModuleBuildProtocolError> {
    let name = archive_string(&header[..100])?;
    let prefix = archive_string(&header[345..500])?;
    let path = if prefix.is_empty() {
        name
    } else if name.is_empty() {
        prefix
    } else {
        format!("{prefix}/{name}")
    };
    if path.is_empty() {
        return Err(ModuleBuildProtocolError::InvalidRequest);
    }
    Ok(PathBuf::from(path))
}

fn archive_string(bytes: &[u8]) -> Result<String, ModuleBuildProtocolError> {
    let value = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
    std::str::from_utf8(value)
        .map(str::to_owned)
        .map_err(|_| ModuleBuildProtocolError::InvalidRequest)
}

fn parse_octal(bytes: &[u8]) -> Result<u64, ModuleBuildProtocolError> {
    let value = archive_string(bytes)?;
    let value = value.trim();
    if value.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(value, 8).map_err(|_| ModuleBuildProtocolError::InvalidRequest)
}

fn copy_exact_archive_bytes(
    archive: &mut File,
    destination: &mut File,
    bytes: u64,
) -> Result<(), ModuleBuildProtocolError> {
    let mut remaining = bytes;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining > 0 {
        let chunk = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| ModuleBuildProtocolError::InvalidLimits)?;
        archive
            .read_exact(&mut buffer[..chunk])
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        destination
            .write_all(&buffer[..chunk])
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        remaining -= u64::try_from(chunk).map_err(|_| ModuleBuildProtocolError::InvalidLimits)?;
    }
    Ok(())
}

fn discard_archive_padding(
    archive: &mut File,
    entry_size: u64,
) -> Result<(), ModuleBuildProtocolError> {
    let padding = (512 - (entry_size % 512)) % 512;
    let mut remaining = padding;
    let mut buffer = [0_u8; 512];
    while remaining > 0 {
        let chunk =
            usize::try_from(remaining).map_err(|_| ModuleBuildProtocolError::InvalidLimits)?;
        archive
            .read_exact(&mut buffer[..chunk])
            .map_err(|error| ModuleBuildProtocolError::Transport(error.to_string()))?;
        remaining -= u64::try_from(chunk).map_err(|_| ModuleBuildProtocolError::InvalidLimits)?;
    }
    Ok(())
}

fn safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}
