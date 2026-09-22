use std::{
    fs,
    path::{Path, PathBuf},
};

use rustok_build_source::{ArchiveLimits, CasArchiveError, CasArchiveStore};
use rustok_modules::ModuleBuildRequest;

const MAX_SOURCE_ARCHIVE_ENTRIES: u32 = 16_384;

/// Deployment-mounted read-only source archive materializer.
///
/// A request may identify source only as `cas://sha256:<hex>`. The matching
/// `<hex>.tar` blob must be present under the fixed source root. This module
/// never receives a storage client or a request-selected filesystem path.
pub struct SourceMaterializer {
    source_store: CasArchiveStore,
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
        let source_store = CasArchiveStore::new(source_root).map_err(|error| error.to_string())?;
        let workspace_root = canonical_directory(workspace_root, "module build workdir")?;
        Ok(Self {
            source_store,
            workspace_root,
        })
    }

    pub async fn materialize(
        &self,
        request: &ModuleBuildRequest,
    ) -> Result<MaterializedSource, SourceMaterializationError> {
        let source_store = self.source_store.clone();
        let workspace_root = self.workspace_root.clone();
        let request_id = request.request_id;
        let source_reference = request.source.reference.clone();
        let source_digest = request.source.digest.clone();
        let disk_limit = request.limits.disk_bytes;
        tokio::task::spawn_blocking(move || {
            materialize_archive(
                source_store,
                workspace_root,
                request_id,
                &source_reference,
                &source_digest,
                disk_limit,
            )
        })
        .await
        .map_err(|error| {
            SourceMaterializationError::Internal(format!("source materializer failed: {error}"))
        })?
    }
}

/// Job-scoped source directory. Dropping it removes all materialized source so
/// a worker process does not retain an untrusted workspace between requests.
pub struct MaterializedSource {
    workspace_root: PathBuf,
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
        remove_owned_job_directory(&self.workspace_root, &self.job_dir);
    }
}

fn materialize_archive(
    source_store: CasArchiveStore,
    workspace_root: PathBuf,
    request_id: uuid::Uuid,
    source_reference: &str,
    source_digest: &str,
    disk_limit: u64,
) -> Result<MaterializedSource, SourceMaterializationError> {
    let limits = ArchiveLimits::new(disk_limit, disk_limit, MAX_SOURCE_ARCHIVE_ENTRIES)
        .map_err(classify_materialization_error)?;
    let job_dir = workspace_root.join(request_id.to_string());
    if path_entry_exists(&job_dir).map_err(SourceMaterializationError::Internal)? {
        remove_owned_job_directory(&workspace_root, &job_dir);
        if path_entry_exists(&job_dir).map_err(SourceMaterializationError::Internal)? {
            return Err(SourceMaterializationError::UnsafeArchive);
        }
    }
    fs::create_dir(&job_dir).map_err(|error| {
        SourceMaterializationError::Internal(format!(
            "materialized job directory could not be created: {error}"
        ))
    })?;
    let source_dir = job_dir.join("source");
    if let Err(error) =
        source_store.materialize(source_reference, source_digest, &source_dir, limits)
    {
        remove_owned_job_directory(&workspace_root, &job_dir);
        return Err(classify_materialization_error(error));
    }
    Ok(MaterializedSource {
        workspace_root,
        job_dir,
        source_dir,
    })
}

fn classify_materialization_error(error: CasArchiveError) -> SourceMaterializationError {
    match error {
        CasArchiveError::DigestMismatch => SourceMaterializationError::DigestMismatch,
        CasArchiveError::ResourceLimit => SourceMaterializationError::ResourceLimit,
        CasArchiveError::InvalidReference
        | CasArchiveError::InvalidDigest
        | CasArchiveError::UnsafeArchive
        | CasArchiveError::DestinationExists => SourceMaterializationError::UnsafeArchive,
        CasArchiveError::Unavailable(message) | CasArchiveError::Io(message) => {
            SourceMaterializationError::Internal(message)
        }
    }
}

fn canonical_directory(path: PathBuf, label: &str) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute"));
    }
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("{label} {} cannot be inspected: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "{label} {} must be a non-symlink directory",
            path.display()
        ));
    }
    fs::canonicalize(&path).map_err(|error| {
        format!(
            "{label} {} cannot be canonicalized: {error}",
            path.display()
        )
    })
}

fn path_entry_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn remove_owned_job_directory(workspace_root: &Path, job_dir: &Path) {
    if job_dir.is_absolute()
        && job_dir.parent() == Some(workspace_root)
        && fs::symlink_metadata(job_dir)
            .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        let _ = fs::remove_dir_all(job_dir);
    }
}
