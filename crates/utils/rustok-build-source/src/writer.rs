use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{ArchiveLimits, COPY_BUFFER_BYTES, USTAR_BLOCK_BYTES};

const FINAL_DESCRIPTOR_FILE: &str = "module-artifact-descriptor.json";
const IGNORED_ROOT_PATHS: &[&str] = &[".git", "target"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceArchiveBuilder {
    limits: ArchiveLimits,
}

/// A data-only regular file accepted for a host-owned source tree.
///
/// The materializer rejects unsafe paths and never accepts filesystem handles,
/// links, permissions, or timestamps from the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceTreeFile {
    pub path: String,
    pub contents: Vec<u8>,
}

/// Shared safe materializer for reviewed source files. It creates one new
/// directory that can then be handed to [`SourceArchiveBuilder`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceTreeMaterializer {
    limits: ArchiveLimits,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceTreeMaterialization {
    pub source_bytes: u64,
    pub entries: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceArchiveReceipt {
    pub source_digest: String,
    pub archive_bytes: u64,
    pub source_bytes: u64,
    pub entries: u32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SourceArchiveError {
    #[error("source root must be an absolute regular non-symlink directory")]
    InvalidSource,
    #[error("source tree contains a link, special file, forbidden path, or unsupported USTAR path")]
    UnsafeSource,
    #[error("source archive exceeds a configured resource limit")]
    ResourceLimit,
    #[error("source archive destination already exists")]
    DestinationExists,
    #[error("source archive destination must be an absolute path outside the source root")]
    InvalidDestination,
    #[error("source archive write failed: {0}")]
    Io(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SourceTreeError {
    #[error("source tree destination must be a new absolute directory under a regular parent")]
    InvalidDestination,
    #[error("source tree destination already exists")]
    DestinationExists,
    #[error("source tree contains an unsafe, duplicate, or forbidden path")]
    UnsafeSource,
    #[error("source tree exceeds a configured resource limit")]
    ResourceLimit,
    #[error("source tree materialization failed: {0}")]
    Io(String),
}

struct SourceFile {
    source_path: PathBuf,
    archive_path: String,
    bytes: u64,
}

impl SourceArchiveBuilder {
    pub fn new(limits: ArchiveLimits) -> Self {
        Self { limits }
    }

    pub fn write(
        &self,
        source_root: &Path,
        destination: &Path,
    ) -> Result<SourceArchiveReceipt, SourceArchiveError> {
        let source_root = canonical_source_root(source_root)?;
        validate_destination(&source_root, destination)?;
        let files = collect_source_files(&source_root, self.limits)?;
        let source_bytes = files.iter().try_fold(0_u64, |total, file| {
            total
                .checked_add(file.bytes)
                .ok_or(SourceArchiveError::ResourceLimit)
        })?;
        let entries = u32::try_from(files.len()).map_err(|_| SourceArchiveError::ResourceLimit)?;
        let mut archive = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    SourceArchiveError::DestinationExists
                } else {
                    io_error(error)
                }
            })?;
        let result = write_archive(&mut archive, &files, self.limits.max_archive_bytes).and_then(
            |archive_bytes| {
                archive.sync_all().map_err(io_error)?;
                Ok(archive_bytes)
            },
        );
        drop(archive);
        let archive_bytes = match result {
            Ok(bytes) => bytes,
            Err(error) => {
                let _ = fs::remove_file(destination);
                return Err(error);
            }
        };
        let source_digest = match hash_archive(destination) {
            Ok(digest) => digest,
            Err(error) => {
                let _ = fs::remove_file(destination);
                return Err(error);
            }
        };
        Ok(SourceArchiveReceipt {
            source_digest,
            archive_bytes,
            source_bytes,
            entries,
        })
    }
}

impl SourceTreeMaterializer {
    pub fn new(limits: ArchiveLimits) -> Self {
        Self { limits }
    }

    /// Validates data-only reviewed source without touching the filesystem.
    /// Hosts use this before persisting or reviewing a source snapshot.
    pub fn validate(
        &self,
        files: &[SourceTreeFile],
    ) -> Result<SourceTreeMaterialization, SourceTreeError> {
        let (_, source_bytes, entries) = collect_tree_files(files, self.limits)?;
        Ok(SourceTreeMaterialization {
            source_bytes,
            entries,
        })
    }

    /// Creates `destination` and writes the reviewed source exactly once. The
    /// destination must not exist; on failure this method removes only that
    /// directory, never a caller-owned pre-existing path.
    pub fn write(
        &self,
        files: &[SourceTreeFile],
        destination: &Path,
    ) -> Result<SourceTreeMaterialization, SourceTreeError> {
        let (files, source_bytes, entries) = collect_tree_files(files, self.limits)?;
        validate_tree_destination(destination)?;
        fs::create_dir(destination).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                SourceTreeError::DestinationExists
            } else {
                tree_io_error(error)
            }
        })?;

        let result = files
            .into_iter()
            .try_for_each(|file| write_tree_file(destination, &file.path, &file.contents));
        match result {
            Ok(()) => Ok(SourceTreeMaterialization {
                source_bytes,
                entries,
            }),
            Err(error) => {
                remove_created_tree(destination);
                Err(error)
            }
        }
    }
}

fn collect_tree_files(
    files: &[SourceTreeFile],
    limits: ArchiveLimits,
) -> Result<(Vec<SourceTreeFile>, u64, u32), SourceTreeError> {
    if files.is_empty() {
        return Err(SourceTreeError::UnsafeSource);
    }
    let mut paths = BTreeSet::new();
    let mut directories = BTreeSet::new();
    let mut source_bytes = 0_u64;
    let mut normalized_files = Vec::with_capacity(files.len());
    for file in files {
        let normalized_path = normalized_tree_path(&file.path)?;
        if !paths.insert(normalized_path.clone()) {
            return Err(SourceTreeError::UnsafeSource);
        }
        source_bytes = source_bytes
            .checked_add(
                u64::try_from(file.contents.len()).map_err(|_| SourceTreeError::ResourceLimit)?,
            )
            .ok_or(SourceTreeError::ResourceLimit)?;
        if source_bytes > limits.max_extracted_bytes {
            return Err(SourceTreeError::ResourceLimit);
        }
        let mut parent = Path::new(&normalized_path).parent();
        while let Some(directory) = parent {
            if directory.as_os_str().is_empty() {
                break;
            }
            directories.insert(directory.to_string_lossy().replace('\\', "/"));
            parent = directory.parent();
        }
        normalized_files.push(SourceTreeFile {
            path: normalized_path,
            contents: file.contents.clone(),
        });
    }
    let entry_count = normalized_files
        .len()
        .checked_add(directories.len())
        .ok_or(SourceTreeError::ResourceLimit)?;
    let entries = u32::try_from(entry_count).map_err(|_| SourceTreeError::ResourceLimit)?;
    if entries > limits.max_entries {
        return Err(SourceTreeError::ResourceLimit);
    }
    normalized_files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok((normalized_files, source_bytes, entries))
}

fn normalized_tree_path(path: &str) -> Result<String, SourceTreeError> {
    if path.is_empty() || path.contains('\\') {
        return Err(SourceTreeError::UnsafeSource);
    }
    let path = Path::new(path);
    let root = path.components().next();
    if !safe_relative_path(path)
        || matches!(root, Some(Component::Normal(name)) if name.to_str().is_some_and(|name| name == ".git" || name == "target"))
        || forbidden_source_path(path)
    {
        return Err(SourceTreeError::UnsafeSource);
    }
    normalized_archive_path(path).map_err(|_| SourceTreeError::UnsafeSource)
}

fn validate_tree_destination(destination: &Path) -> Result<(), SourceTreeError> {
    if !destination.is_absolute() {
        return Err(SourceTreeError::InvalidDestination);
    }
    match fs::symlink_metadata(destination) {
        Ok(_) => return Err(SourceTreeError::DestinationExists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(tree_io_error(error)),
    }
    let parent = destination
        .parent()
        .ok_or(SourceTreeError::InvalidDestination)?;
    let metadata = fs::symlink_metadata(parent).map_err(tree_io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(SourceTreeError::InvalidDestination);
    }
    Ok(())
}

fn write_tree_file(
    destination: &Path,
    relative_path: &str,
    contents: &[u8],
) -> Result<(), SourceTreeError> {
    let relative = Path::new(relative_path);
    let parent = relative.parent().ok_or(SourceTreeError::UnsafeSource)?;
    let mut directory = destination.to_path_buf();
    for component in parent.components() {
        let Component::Normal(component) = component else {
            return Err(SourceTreeError::UnsafeSource);
        };
        directory.push(component);
        match fs::create_dir(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(tree_io_error(error)),
        }
        let metadata = fs::symlink_metadata(&directory).map_err(tree_io_error)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(SourceTreeError::UnsafeSource);
        }
    }
    let path = destination.join(relative);
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(tree_io_error)?;
    output.write_all(contents).map_err(tree_io_error)?;
    output.sync_all().map_err(tree_io_error)
}

fn remove_created_tree(path: &Path) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() || metadata.is_file() {
        let _ = fs::remove_file(path);
    } else if metadata.is_dir() {
        let _ = fs::remove_dir_all(path);
    }
}

fn tree_io_error(error: impl std::fmt::Display) -> SourceTreeError {
    SourceTreeError::Io(error.to_string())
}

fn canonical_source_root(source_root: &Path) -> Result<PathBuf, SourceArchiveError> {
    if !source_root.is_absolute() {
        return Err(SourceArchiveError::InvalidSource);
    }
    let metadata =
        fs::symlink_metadata(source_root).map_err(|_| SourceArchiveError::InvalidSource)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(SourceArchiveError::InvalidSource);
    }
    fs::canonicalize(source_root).map_err(io_error)
}

fn validate_destination(source_root: &Path, destination: &Path) -> Result<(), SourceArchiveError> {
    if !destination.is_absolute() {
        return Err(SourceArchiveError::InvalidDestination);
    }
    match fs::symlink_metadata(destination) {
        Ok(_) => return Err(SourceArchiveError::DestinationExists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(error)),
    }
    let parent = destination
        .parent()
        .ok_or(SourceArchiveError::InvalidDestination)?;
    let metadata = fs::symlink_metadata(parent).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(SourceArchiveError::InvalidDestination);
    }
    let parent = fs::canonicalize(parent).map_err(io_error)?;
    if parent.starts_with(source_root) {
        return Err(SourceArchiveError::InvalidDestination);
    }
    Ok(())
}

fn collect_source_files(
    source_root: &Path,
    limits: ArchiveLimits,
) -> Result<Vec<SourceFile>, SourceArchiveError> {
    let mut directories = vec![source_root.to_path_buf()];
    let mut files = Vec::new();
    let mut observed_entries = 0_u32;
    let mut source_bytes = 0_u64;
    while let Some(directory) = directories.pop() {
        let entries = fs::read_dir(&directory).map_err(io_error)?;
        for entry in entries {
            let entry = entry.map_err(io_error)?;
            let path = entry.path();
            let relative = path
                .strip_prefix(source_root)
                .map_err(|_| SourceArchiveError::UnsafeSource)?;
            if !safe_relative_path(relative) {
                return Err(SourceArchiveError::UnsafeSource);
            }
            let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
            if metadata.file_type().is_symlink() {
                return Err(SourceArchiveError::UnsafeSource);
            }
            if ignored_root_path(relative, &metadata) {
                continue;
            }
            if forbidden_source_path(relative) {
                return Err(SourceArchiveError::UnsafeSource);
            }
            observed_entries = observed_entries
                .checked_add(1)
                .ok_or(SourceArchiveError::ResourceLimit)?;
            if observed_entries > limits.max_entries {
                return Err(SourceArchiveError::ResourceLimit);
            }
            if metadata.is_dir() {
                directories.push(path);
            } else if metadata.is_file() {
                source_bytes = source_bytes
                    .checked_add(metadata.len())
                    .ok_or(SourceArchiveError::ResourceLimit)?;
                if source_bytes > limits.max_extracted_bytes {
                    return Err(SourceArchiveError::ResourceLimit);
                }
                let archive_path = normalized_archive_path(relative)?;
                files.push(SourceFile {
                    source_path: path,
                    archive_path,
                    bytes: metadata.len(),
                });
            } else {
                return Err(SourceArchiveError::UnsafeSource);
            }
        }
    }
    if files.is_empty() {
        return Err(SourceArchiveError::UnsafeSource);
    }
    files.sort_by(|left, right| left.archive_path.cmp(&right.archive_path));
    Ok(files)
}

fn ignored_root_path(relative: &Path, metadata: &fs::Metadata) -> bool {
    relative.components().count() == 1
        && relative.to_str().is_some_and(|name| {
            IGNORED_ROOT_PATHS.contains(&name) && (name == ".git" || metadata.is_dir())
        })
}

fn forbidden_source_path(relative: &Path) -> bool {
    let normalized = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    matches!(
        normalized.as_str(),
        FINAL_DESCRIPTOR_FILE | ".cargo/config" | ".cargo/config.toml"
    )
}

fn safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn normalized_archive_path(path: &Path) -> Result<String, SourceArchiveError> {
    let components = path
        .components()
        .map(|component| match component {
            Component::Normal(value) => value.to_str().ok_or(SourceArchiveError::UnsafeSource),
            _ => Err(SourceArchiveError::UnsafeSource),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let path = components.join("/");
    split_ustar_path(&path)?;
    Ok(path)
}

fn write_archive(
    archive: &mut File,
    files: &[SourceFile],
    maximum_bytes: u64,
) -> Result<u64, SourceArchiveError> {
    let mut written = 0_u64;
    for source in files {
        let header = ustar_header(&source.archive_path, source.bytes)?;
        write_bounded(archive, &header, &mut written, maximum_bytes)?;
        let mut input = File::open(&source.source_path).map_err(io_error)?;
        let metadata = input.metadata().map_err(io_error)?;
        if !metadata.is_file() || metadata.len() != source.bytes {
            return Err(SourceArchiveError::UnsafeSource);
        }
        let mut remaining = source.bytes;
        let mut buffer = [0_u8; COPY_BUFFER_BYTES];
        while remaining > 0 {
            let chunk = usize::try_from(remaining.min(buffer.len() as u64))
                .map_err(|_| SourceArchiveError::ResourceLimit)?;
            input
                .read_exact(&mut buffer[..chunk])
                .map_err(|_| SourceArchiveError::UnsafeSource)?;
            write_bounded(archive, &buffer[..chunk], &mut written, maximum_bytes)?;
            remaining -= u64::try_from(chunk).map_err(|_| SourceArchiveError::ResourceLimit)?;
        }
        let mut extra = [0_u8; 1];
        if input.read(&mut extra).map_err(io_error)? != 0
            || input.metadata().map_err(io_error)?.len() != source.bytes
        {
            return Err(SourceArchiveError::UnsafeSource);
        }
        let padding = (USTAR_BLOCK_BYTES as u64 - (source.bytes % USTAR_BLOCK_BYTES as u64))
            % USTAR_BLOCK_BYTES as u64;
        if padding > 0 {
            let padding =
                usize::try_from(padding).map_err(|_| SourceArchiveError::ResourceLimit)?;
            write_bounded(
                archive,
                &[0_u8; USTAR_BLOCK_BYTES][..padding],
                &mut written,
                maximum_bytes,
            )?;
        }
    }
    write_bounded(
        archive,
        &[0_u8; USTAR_BLOCK_BYTES * 2],
        &mut written,
        maximum_bytes,
    )?;
    Ok(written)
}

fn ustar_header(path: &str, bytes: u64) -> Result<[u8; USTAR_BLOCK_BYTES], SourceArchiveError> {
    let (prefix, name) = split_ustar_path(path)?;
    let mut header = [0_u8; USTAR_BLOCK_BYTES];
    copy_field(&mut header[0..100], name.as_bytes())?;
    write_octal(&mut header[100..108], 0o644)?;
    write_octal(&mut header[108..116], 0)?;
    write_octal(&mut header[116..124], 0)?;
    write_octal(&mut header[124..136], bytes)?;
    write_octal(&mut header[136..148], 0)?;
    header[148..156].fill(b' ');
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    copy_field(&mut header[345..500], prefix.as_bytes())?;
    let checksum = header.iter().map(|byte| u64::from(*byte)).sum::<u64>();
    let checksum = format!("{checksum:06o}");
    if checksum.len() != 6 {
        return Err(SourceArchiveError::UnsafeSource);
    }
    header[148..154].copy_from_slice(checksum.as_bytes());
    header[154] = 0;
    header[155] = b' ';
    Ok(header)
}

fn split_ustar_path(path: &str) -> Result<(&str, &str), SourceArchiveError> {
    if path.len() <= 100 {
        return Ok(("", path));
    }
    let mut selected = None;
    for (index, _) in path.match_indices('/') {
        let prefix = &path[..index];
        let name = &path[index + 1..];
        if prefix.len() <= 155 && !name.is_empty() && name.len() <= 100 {
            selected = Some((prefix, name));
        }
    }
    selected.ok_or(SourceArchiveError::UnsafeSource)
}

fn copy_field(field: &mut [u8], value: &[u8]) -> Result<(), SourceArchiveError> {
    if value.len() > field.len() {
        return Err(SourceArchiveError::UnsafeSource);
    }
    field[..value.len()].copy_from_slice(value);
    Ok(())
}

fn write_octal(field: &mut [u8], value: u64) -> Result<(), SourceArchiveError> {
    let field_len = field.len();
    let encoded = format!("{value:o}");
    if encoded.len() + 1 > field_len {
        return Err(SourceArchiveError::ResourceLimit);
    }
    field.fill(b'0');
    let start = field_len - encoded.len() - 1;
    field[start..field_len - 1].copy_from_slice(encoded.as_bytes());
    field[field_len - 1] = 0;
    Ok(())
}

fn write_bounded(
    output: &mut File,
    bytes: &[u8],
    written: &mut u64,
    maximum: u64,
) -> Result<(), SourceArchiveError> {
    let next = written
        .checked_add(u64::try_from(bytes.len()).map_err(|_| SourceArchiveError::ResourceLimit)?)
        .ok_or(SourceArchiveError::ResourceLimit)?;
    if next > maximum {
        return Err(SourceArchiveError::ResourceLimit);
    }
    output.write_all(bytes).map_err(io_error)?;
    *written = next;
    Ok(())
}

fn hash_archive(path: &Path) -> Result<String, SourceArchiveError> {
    let mut archive = File::open(path).map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = archive.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

fn io_error(error: impl std::fmt::Display) -> SourceArchiveError {
    SourceArchiveError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CasArchiveError, CasArchiveStore, SourceArchiveInspector};
    use uuid::Uuid;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn create() -> Self {
            let path =
                std::env::temp_dir().join(format!("rustok-source-archive-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn limits() -> ArchiveLimits {
        ArchiveLimits::new(1024 * 1024, 1024 * 1024, 128).expect("limits")
    }

    #[test]
    fn reviewed_tree_materialization_round_trips_through_the_shared_archive_writer() {
        let root = TestDirectory::create();
        let source = root.0.join("source");
        let archive = root.0.join("source.tar");
        let materialized = SourceTreeMaterializer::new(limits())
            .write(
                &[
                    SourceTreeFile {
                        path: "src/lib.rs".to_string(),
                        contents: b"pub fn run() {}\n".to_vec(),
                    },
                    SourceTreeFile {
                        path: "Cargo.toml".to_string(),
                        contents: b"[package]\nname = \"sample\"\n".to_vec(),
                    },
                ],
                &source,
            )
            .expect("materialized source tree");
        assert_eq!(materialized.entries, 3);
        assert_eq!(materialized.source_bytes, 42);

        let packaged = SourceArchiveBuilder::new(limits())
            .write(&source, &archive)
            .expect("source archive");
        assert_eq!(
            SourceArchiveInspector::new(limits())
                .inspect(&archive)
                .expect("strict archive inspection")
                .source_digest,
            packaged.source_digest
        );
    }

    #[test]
    fn reviewed_tree_rejects_unsafe_and_duplicate_paths_without_creating_a_destination() {
        let root = TestDirectory::create();
        let unsafe_source = root.0.join("unsafe-source");
        assert_eq!(
            SourceTreeMaterializer::new(limits()).write(
                &[SourceTreeFile {
                    path: ".cargo/config.toml".to_string(),
                    contents: b"[net]\noffline = false\n".to_vec(),
                },],
                &unsafe_source,
            ),
            Err(SourceTreeError::UnsafeSource)
        );
        assert!(!unsafe_source.exists());

        let duplicate_source = root.0.join("duplicate-source");
        assert_eq!(
            SourceTreeMaterializer::new(limits()).write(
                &[
                    SourceTreeFile {
                        path: "src/lib.rs".to_string(),
                        contents: b"pub fn first() {}\n".to_vec(),
                    },
                    SourceTreeFile {
                        path: "src/lib.rs".to_string(),
                        contents: b"pub fn second() {}\n".to_vec(),
                    },
                ],
                &duplicate_source,
            ),
            Err(SourceTreeError::UnsafeSource)
        );
        assert!(!duplicate_source.exists());
    }

    #[test]
    fn deterministic_archive_round_trips_through_the_strict_materializer() {
        let workspace = TestDirectory::create();
        let source = workspace.0.join("source");
        let archives = workspace.0.join("archives");
        fs::create_dir(&source).expect("source");
        fs::create_dir(&archives).expect("archives");
        fs::create_dir(source.join("src")).expect("src");
        fs::write(source.join("Cargo.toml"), b"[package]\nname='sample'\n").expect("manifest");
        fs::write(source.join("src/lib.rs"), b"pub fn sample() {}\n").expect("source file");
        fs::create_dir(source.join("target")).expect("target");
        fs::write(source.join("target/ignored"), b"ignored").expect("ignored output");
        fs::write(source.join(".git"), b"gitdir: outside-the-project\n").expect("Git marker");

        let first = archives.join("first.tar");
        let second = archives.join("second.tar");
        let builder = SourceArchiveBuilder::new(limits());
        let first_receipt = builder.write(&source, &first).expect("first archive");
        let second_receipt = builder.write(&source, &second).expect("second archive");
        assert_eq!(first_receipt.source_digest, second_receipt.source_digest);
        assert_eq!(
            fs::read(&first).expect("first"),
            fs::read(&second).expect("second")
        );
        let inspection = SourceArchiveInspector::new(limits())
            .inspect(&second)
            .expect("inspect canonical archive");
        assert_eq!(inspection.source_digest, second_receipt.source_digest);
        assert_eq!(inspection.extracted_bytes, second_receipt.source_bytes);
        assert_eq!(inspection.entries, second_receipt.entries);

        let digest = first_receipt
            .source_digest
            .strip_prefix("sha256:")
            .expect("digest prefix");
        let cas_archive = archives.join(format!("{digest}.tar"));
        fs::rename(first, &cas_archive).expect("name CAS archive");
        let destination = workspace.0.join("materialized");
        CasArchiveStore::new(archives)
            .expect("CAS store")
            .materialize(
                &format!("cas://{}", first_receipt.source_digest),
                &first_receipt.source_digest,
                &destination,
                limits(),
            )
            .expect("round-trip materialization");
        assert_eq!(
            fs::read(destination.join("src/lib.rs")).expect("materialized source"),
            b"pub fn sample() {}\n"
        );
        assert!(!destination.join("target").exists());
        assert!(!destination.join(".git").exists());
    }

    #[test]
    fn final_descriptor_and_source_cargo_config_are_rejected() {
        let workspace = TestDirectory::create();
        let source = workspace.0.join("source");
        fs::create_dir(&source).expect("source");
        fs::write(source.join(FINAL_DESCRIPTOR_FILE), b"{}").expect("descriptor");
        let archive = workspace.0.join("source.tar");
        assert!(matches!(
            SourceArchiveBuilder::new(limits()).write(&source, &archive),
            Err(SourceArchiveError::UnsafeSource)
        ));

        fs::remove_file(source.join(FINAL_DESCRIPTOR_FILE)).expect("remove descriptor");
        fs::create_dir(source.join(".cargo")).expect("Cargo config directory");
        fs::write(
            source.join(".cargo/config.toml"),
            b"[net]\noffline = false\n",
        )
        .expect("Cargo config");
        assert!(matches!(
            SourceArchiveBuilder::new(limits()).write(&source, &archive),
            Err(SourceArchiveError::UnsafeSource)
        ));
    }

    #[test]
    fn inspector_rejects_non_zero_file_padding() {
        let workspace = TestDirectory::create();
        let source = workspace.0.join("source");
        fs::create_dir(&source).expect("source");
        let contents = b"not block aligned\n";
        fs::write(source.join("source.txt"), contents).expect("source file");
        let archive = workspace.0.join("source.tar");
        SourceArchiveBuilder::new(limits())
            .write(&source, &archive)
            .expect("archive");

        let mut bytes = fs::read(&archive).expect("read archive");
        bytes[USTAR_BLOCK_BYTES + contents.len()] = 1;
        fs::write(&archive, bytes).expect("corrupt padding");

        assert_eq!(
            SourceArchiveInspector::new(limits()).inspect(&archive),
            Err(CasArchiveError::UnsafeArchive)
        );
    }
}
