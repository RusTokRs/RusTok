//! Deterministic immutable source archives shared by authoring and build workers.

mod writer;

use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use writer::{
    SourceArchiveBuilder, SourceArchiveError, SourceArchiveReceipt, SourceTreeError,
    SourceTreeFile, SourceTreeMaterialization, SourceTreeMaterializer,
};

const USTAR_BLOCK_BYTES: usize = 512;
const COPY_BUFFER_BYTES: usize = 64 * 1024;
const MAX_ARCHIVE_ENTRIES: u32 = 65_536;
static SOURCE_UPLOAD_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveLimits {
    pub max_archive_bytes: u64,
    pub max_extracted_bytes: u64,
    pub max_entries: u32,
}

impl ArchiveLimits {
    pub fn new(
        max_archive_bytes: u64,
        max_extracted_bytes: u64,
        max_entries: u32,
    ) -> Result<Self, CasArchiveError> {
        if max_archive_bytes == 0
            || max_extracted_bytes == 0
            || max_entries == 0
            || max_entries > MAX_ARCHIVE_ENTRIES
        {
            return Err(CasArchiveError::ResourceLimit);
        }
        Ok(Self {
            max_archive_bytes,
            max_extracted_bytes,
            max_entries,
        })
    }
}

#[derive(Clone, Debug)]
pub struct CasArchiveStore {
    root: PathBuf,
}

/// Control-plane writer for the deployment source CAS consumed read-only by
/// isolated build jobs.
#[derive(Clone, Debug)]
pub struct CasArchivePublisher {
    root: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CasArchiveReceipt {
    pub source_digest: String,
    pub archive_bytes: u64,
    pub extracted_bytes: u64,
    pub entries: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CasArchivePublishReceipt {
    pub source_digest: String,
    pub archive_bytes: u64,
    pub extracted_bytes: u64,
    pub entries: u32,
    pub created: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceArchiveInspector {
    limits: ArchiveLimits,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceArchiveInspection {
    pub source_digest: String,
    pub archive_bytes: u64,
    pub extracted_bytes: u64,
    pub entries: u32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CasArchiveError {
    #[error("CAS source reference is invalid")]
    InvalidReference,
    #[error("CAS source digest is invalid")]
    InvalidDigest,
    #[error("CAS source archive is unavailable: {0}")]
    Unavailable(String),
    #[error("CAS source archive digest does not match its immutable identity")]
    DigestMismatch,
    #[error("CAS source archive violates the strict USTAR contract")]
    UnsafeArchive,
    #[error("CAS source archive exceeds a materialization resource limit")]
    ResourceLimit,
    #[error("CAS source destination already exists")]
    DestinationExists,
    #[error("CAS source materialization failed: {0}")]
    Io(String),
}

impl CasArchiveStore {
    pub fn new(root: PathBuf) -> Result<Self, CasArchiveError> {
        if !root.is_absolute() {
            return Err(CasArchiveError::Io(
                "CAS source root must be absolute".to_string(),
            ));
        }
        validate_directory(&root)?;
        let root = fs::canonicalize(&root).map_err(io_error)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn materialize(
        &self,
        source_reference: &str,
        source_digest: &str,
        destination: &Path,
        limits: ArchiveLimits,
    ) -> Result<CasArchiveReceipt, CasArchiveError> {
        let digest_hex = validate_identity(source_reference, source_digest)?;
        validate_destination(destination)?;
        let direct_path = self.root.join(digest_hex);
        let archive_path = if direct_path.exists() {
            direct_path
        } else {
            self.root.join(format!("{digest_hex}.tar"))
        };
        let archive_metadata = fs::symlink_metadata(&archive_path)
            .map_err(|error| CasArchiveError::Unavailable(error.to_string()))?;
        if archive_metadata.file_type().is_symlink() || !archive_metadata.is_file() {
            return Err(CasArchiveError::UnsafeArchive);
        }
        if archive_metadata.len() > limits.max_archive_bytes {
            return Err(CasArchiveError::ResourceLimit);
        }
        verify_archive_digest(&archive_path, source_digest)?;
        fs::create_dir(destination).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                CasArchiveError::DestinationExists
            } else {
                io_error(error)
            }
        })?;
        match scan_safe_archive(&archive_path, Some(destination), limits) {
            Ok((extracted_bytes, entries)) => Ok(CasArchiveReceipt {
                source_digest: source_digest.to_string(),
                archive_bytes: archive_metadata.len(),
                extracted_bytes,
                entries,
            }),
            Err(error) => {
                remove_created_destination(destination);
                Err(error)
            }
        }
    }
}

impl CasArchivePublisher {
    pub fn new(root: PathBuf) -> Result<Self, CasArchiveError> {
        if !root.is_absolute() {
            return Err(CasArchiveError::Io(
                "CAS source root must be absolute".to_string(),
            ));
        }
        validate_directory(&root)?;
        let root = fs::canonicalize(&root).map_err(io_error)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Publishes one already packaged source archive without ever exposing
    /// bytes under an unverified digest identity. The temporary file and final
    /// object are on the same filesystem; a hard-link create is the atomic
    /// no-replace commit, so concurrent equal uploads converge and a different
    /// object can never be overwritten.
    pub fn publish(
        &self,
        archive_path: &Path,
        expected_digest: &str,
        limits: ArchiveLimits,
    ) -> Result<CasArchivePublishReceipt, CasArchiveError> {
        let digest_hex = validate_identity(&format!("cas://{expected_digest}"), expected_digest)?;
        let inspection = SourceArchiveInspector::new(limits).inspect(archive_path)?;
        if inspection.source_digest != expected_digest {
            return Err(CasArchiveError::DigestMismatch);
        }

        let direct_path = self.root.join(digest_hex);
        let tar_path = self.root.join(format!("{digest_hex}.tar"));
        let destination = if path_entry_exists(&direct_path)? {
            return validate_published_archive(&direct_path, expected_digest, limits, false);
        } else if path_entry_exists(&tar_path)? {
            return validate_published_archive(&tar_path, expected_digest, limits, false);
        } else {
            direct_path
        };

        let (temporary_path, mut temporary) = create_upload_file(&self.root)?;
        let copied = copy_and_hash_archive(archive_path, &mut temporary, limits.max_archive_bytes);
        let copied_digest = match copied {
            Ok(copied_digest) => copied_digest,
            Err(error) => {
                remove_upload_file(&temporary_path);
                return Err(error);
            }
        };
        if copied_digest != expected_digest {
            remove_upload_file(&temporary_path);
            return Err(CasArchiveError::DigestMismatch);
        }
        if let Err(error) = temporary.sync_all().map_err(io_error) {
            remove_upload_file(&temporary_path);
            return Err(error);
        }
        drop(temporary);

        match fs::hard_link(&temporary_path, &destination) {
            Ok(()) => {
                let _ = fs::hard_link(&destination, &tar_path);
                fs::remove_file(&temporary_path).map_err(io_error)?;
                Ok(CasArchivePublishReceipt {
                    source_digest: inspection.source_digest,
                    archive_bytes: inspection.archive_bytes,
                    extracted_bytes: inspection.extracted_bytes,
                    entries: inspection.entries,
                    created: true,
                })
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                fs::remove_file(&temporary_path).map_err(io_error)?;
                validate_published_archive(&destination, expected_digest, limits, false)
            }
            Err(error) => {
                remove_upload_file(&temporary_path);
                Err(io_error(error))
            }
        }
    }
}

impl SourceArchiveInspector {
    pub fn new(limits: ArchiveLimits) -> Self {
        Self { limits }
    }

    pub fn inspect(&self, archive_path: &Path) -> Result<SourceArchiveInspection, CasArchiveError> {
        if !archive_path.is_absolute() {
            return Err(CasArchiveError::Io(
                "source archive path must be absolute".to_string(),
            ));
        }
        let metadata = fs::symlink_metadata(archive_path).map_err(io_error)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(CasArchiveError::UnsafeArchive);
        }
        if metadata.len() > self.limits.max_archive_bytes {
            return Err(CasArchiveError::ResourceLimit);
        }
        let source_digest = hash_archive_digest(archive_path)?;
        let (extracted_bytes, entries) = scan_safe_archive(archive_path, None, self.limits)?;
        Ok(SourceArchiveInspection {
            source_digest,
            archive_bytes: metadata.len(),
            extracted_bytes,
            entries,
        })
    }
}

fn validate_identity<'a>(
    source_reference: &str,
    source_digest: &'a str,
) -> Result<&'a str, CasArchiveError> {
    let digest_hex = source_digest
        .strip_prefix("sha256:")
        .ok_or(CasArchiveError::InvalidDigest)?;
    if digest_hex.len() != 64
        || !digest_hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(CasArchiveError::InvalidDigest);
    }
    if source_reference != format!("cas://{source_digest}") {
        return Err(CasArchiveError::InvalidReference);
    }
    Ok(digest_hex)
}

fn validate_destination(destination: &Path) -> Result<(), CasArchiveError> {
    if !destination.is_absolute() {
        return Err(CasArchiveError::Io(
            "CAS source destination must be absolute".to_string(),
        ));
    }
    match fs::symlink_metadata(destination) {
        Ok(_) => return Err(CasArchiveError::DestinationExists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(error)),
    }
    let parent = destination
        .parent()
        .ok_or_else(|| CasArchiveError::Io("CAS source destination has no parent".to_string()))?;
    validate_directory(parent)
}

fn validate_directory(path: &Path) -> Result<(), CasArchiveError> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CasArchiveError::Io(
            "CAS source path must be a non-symlink directory".to_string(),
        ));
    }
    Ok(())
}

fn path_entry_exists(path: &Path) -> Result<bool, CasArchiveError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(error)),
    }
}

fn validate_published_archive(
    path: &Path,
    expected_digest: &str,
    limits: ArchiveLimits,
    created: bool,
) -> Result<CasArchivePublishReceipt, CasArchiveError> {
    let inspection = SourceArchiveInspector::new(limits).inspect(path)?;
    if inspection.source_digest != expected_digest {
        return Err(CasArchiveError::DigestMismatch);
    }
    Ok(CasArchivePublishReceipt {
        source_digest: inspection.source_digest,
        archive_bytes: inspection.archive_bytes,
        extracted_bytes: inspection.extracted_bytes,
        entries: inspection.entries,
        created,
    })
}

fn create_upload_file(root: &Path) -> Result<(PathBuf, File), CasArchiveError> {
    for _ in 0..64 {
        let sequence = SOURCE_UPLOAD_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = root.join(format!(
            ".source-upload-{}-{sequence}.tmp",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error(error)),
        }
    }
    Err(CasArchiveError::Io(
        "could not reserve a unique CAS source upload path".to_string(),
    ))
}

fn copy_and_hash_archive(
    source_path: &Path,
    destination: &mut File,
    maximum_bytes: u64,
) -> Result<String, CasArchiveError> {
    let metadata = fs::symlink_metadata(source_path).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CasArchiveError::UnsafeArchive);
    }
    let mut source = File::open(source_path).map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut copied = 0_u64;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = source.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        copied = copied
            .checked_add(u64::try_from(read).map_err(|_| CasArchiveError::ResourceLimit)?)
            .ok_or(CasArchiveError::ResourceLimit)?;
        if copied > maximum_bytes {
            return Err(CasArchiveError::ResourceLimit);
        }
        destination.write_all(&buffer[..read]).map_err(io_error)?;
        hasher.update(&buffer[..read]);
    }
    if copied != metadata.len() {
        return Err(CasArchiveError::DigestMismatch);
    }
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

fn remove_upload_file(path: &Path) {
    let _ = fs::remove_file(path);
}

fn verify_archive_digest(path: &Path, expected: &str) -> Result<(), CasArchiveError> {
    let actual = hash_archive_digest(path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(CasArchiveError::DigestMismatch)
    }
}

fn hash_archive_digest(path: &Path) -> Result<String, CasArchiveError> {
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

fn scan_safe_archive(
    archive_path: &Path,
    destination_root: Option<&Path>,
    limits: ArchiveLimits,
) -> Result<(u64, u32), CasArchiveError> {
    let mut archive = File::open(archive_path).map_err(io_error)?;
    let mut extracted_bytes = 0_u64;
    let mut entries = 0_u32;
    let mut declared_paths = HashSet::new();
    loop {
        let header = read_block(&mut archive)?;
        if header.iter().all(|byte| *byte == 0) {
            validate_archive_terminator(&mut archive)?;
            return Ok((extracted_bytes, entries));
        }
        if &header[257..263] != b"ustar\0" {
            return Err(CasArchiveError::UnsafeArchive);
        }
        validate_ustar_checksum(&header)?;
        let relative_path = ustar_path(&header)?;
        if !safe_relative_path(&relative_path) || !declared_paths.insert(relative_path.clone()) {
            return Err(CasArchiveError::UnsafeArchive);
        }
        entries = entries
            .checked_add(1)
            .ok_or(CasArchiveError::ResourceLimit)?;
        if entries > limits.max_entries {
            return Err(CasArchiveError::ResourceLimit);
        }
        let entry_type = header[156];
        let entry_size = parse_octal(&header[124..136])?;
        if entry_type == b'5' {
            if entry_size != 0 {
                return Err(CasArchiveError::UnsafeArchive);
            }
            if let Some(destination_root) = destination_root {
                let destination = archive_destination(destination_root, &relative_path)?;
                create_archive_directory(&destination)?;
            }
            continue;
        }
        if entry_type != 0 && entry_type != b'0' {
            return Err(CasArchiveError::UnsafeArchive);
        }
        extracted_bytes = extracted_bytes
            .checked_add(entry_size)
            .ok_or(CasArchiveError::ResourceLimit)?;
        if extracted_bytes > limits.max_extracted_bytes {
            return Err(CasArchiveError::ResourceLimit);
        }
        if let Some(destination_root) = destination_root {
            let destination = archive_destination(destination_root, &relative_path)?;
            let parent = destination.parent().ok_or(CasArchiveError::UnsafeArchive)?;
            create_archive_directory(parent)?;
            let mut destination_file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
                .map_err(|_| CasArchiveError::UnsafeArchive)?;
            copy_exact_archive_bytes(&mut archive, &mut destination_file, entry_size)?;
            destination_file.sync_all().map_err(io_error)?;
        } else {
            copy_exact_archive_bytes(&mut archive, &mut std::io::sink(), entry_size)?;
        }
        discard_archive_padding(&mut archive, entry_size)?;
    }
}

fn archive_destination(
    destination_root: &Path,
    relative_path: &Path,
) -> Result<PathBuf, CasArchiveError> {
    let destination = destination_root.join(relative_path);
    if destination.starts_with(destination_root) {
        Ok(destination)
    } else {
        Err(CasArchiveError::UnsafeArchive)
    }
}

fn read_block(archive: &mut File) -> Result<[u8; USTAR_BLOCK_BYTES], CasArchiveError> {
    let mut block = [0_u8; USTAR_BLOCK_BYTES];
    archive.read_exact(&mut block).map_err(|error| {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            CasArchiveError::UnsafeArchive
        } else {
            io_error(error)
        }
    })?;
    Ok(block)
}

fn validate_archive_terminator(archive: &mut File) -> Result<(), CasArchiveError> {
    if !read_block(archive)?.iter().all(|byte| *byte == 0) {
        return Err(CasArchiveError::UnsafeArchive);
    }
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = archive.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            return Ok(());
        }
        if buffer[..read].iter().any(|byte| *byte != 0) {
            return Err(CasArchiveError::UnsafeArchive);
        }
    }
}

fn validate_ustar_checksum(header: &[u8; USTAR_BLOCK_BYTES]) -> Result<(), CasArchiveError> {
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
    if expected == actual {
        Ok(())
    } else {
        Err(CasArchiveError::UnsafeArchive)
    }
}

fn ustar_path(header: &[u8; USTAR_BLOCK_BYTES]) -> Result<PathBuf, CasArchiveError> {
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
        return Err(CasArchiveError::UnsafeArchive);
    }
    Ok(PathBuf::from(path))
}

fn archive_string(bytes: &[u8]) -> Result<String, CasArchiveError> {
    let value = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
    std::str::from_utf8(value)
        .map(str::to_owned)
        .map_err(|_| CasArchiveError::UnsafeArchive)
}

fn parse_octal(bytes: &[u8]) -> Result<u64, CasArchiveError> {
    let value = archive_string(bytes)?;
    let value = value.trim();
    if value.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(value, 8).map_err(|_| CasArchiveError::UnsafeArchive)
}

fn copy_exact_archive_bytes(
    archive: &mut File,
    destination: &mut impl Write,
    bytes: u64,
) -> Result<(), CasArchiveError> {
    let mut remaining = bytes;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    while remaining > 0 {
        let chunk = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| CasArchiveError::ResourceLimit)?;
        archive
            .read_exact(&mut buffer[..chunk])
            .map_err(|_| CasArchiveError::UnsafeArchive)?;
        destination.write_all(&buffer[..chunk]).map_err(io_error)?;
        remaining -= u64::try_from(chunk).map_err(|_| CasArchiveError::ResourceLimit)?;
    }
    Ok(())
}

fn discard_archive_padding(archive: &mut File, entry_size: u64) -> Result<(), CasArchiveError> {
    let padding = (USTAR_BLOCK_BYTES as u64 - (entry_size % USTAR_BLOCK_BYTES as u64))
        % USTAR_BLOCK_BYTES as u64;
    let mut remaining = padding;
    let mut buffer = [0_u8; USTAR_BLOCK_BYTES];
    while remaining > 0 {
        let chunk = usize::try_from(remaining).map_err(|_| CasArchiveError::ResourceLimit)?;
        archive
            .read_exact(&mut buffer[..chunk])
            .map_err(|_| CasArchiveError::UnsafeArchive)?;
        if buffer[..chunk].iter().any(|byte| *byte != 0) {
            return Err(CasArchiveError::UnsafeArchive);
        }
        remaining -= u64::try_from(chunk).map_err(|_| CasArchiveError::ResourceLimit)?;
    }
    Ok(())
}

fn create_archive_directory(path: &Path) -> Result<(), CasArchiveError> {
    fs::create_dir_all(path).map_err(io_error)?;
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CasArchiveError::UnsafeArchive);
    }
    Ok(())
}

fn safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn remove_created_destination(destination: &Path) {
    if destination.is_absolute()
        && fs::symlink_metadata(destination)
            .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        let _ = fs::remove_dir_all(destination);
    }
}

fn io_error(error: impl std::fmt::Display) -> CasArchiveError {
    CasArchiveError::Io(error.to_string())
}

#[cfg(test)]
mod publisher_tests {
    use std::fs;

    use uuid::Uuid;

    use super::*;

    fn limits() -> ArchiveLimits {
        ArchiveLimits::new(1024 * 1024, 1024 * 1024, 128).expect("test limits")
    }

    #[test]
    fn publisher_commits_once_and_revalidates_existing_content() {
        let root = std::env::temp_dir().join(format!("rustok-source-cas-{}", Uuid::new_v4()));
        let source = root.join("source");
        let cas = root.join("cas");
        let archive = root.join("source.tar");
        fs::create_dir_all(&source).expect("source directory");
        fs::create_dir(&cas).expect("CAS directory");
        fs::write(source.join("source.txt"), b"immutable source\n").expect("source file");
        let packaged = SourceArchiveBuilder::new(limits())
            .write(&source, &archive)
            .expect("source archive");
        let publisher = CasArchivePublisher::new(cas.clone()).expect("publisher");

        let first = publisher
            .publish(&archive, &packaged.source_digest, limits())
            .expect("first publication");
        let repeated = publisher
            .publish(&archive, &packaged.source_digest, limits())
            .expect("repeated publication");

        assert!(first.created);
        assert!(!repeated.created);
        assert_eq!(first.source_digest, repeated.source_digest);
        assert_eq!(first.archive_bytes, repeated.archive_bytes);

        let digest_hex = packaged
            .source_digest
            .strip_prefix("sha256:")
            .expect("digest prefix");
        fs::write(cas.join(format!("{digest_hex}.tar")), b"corrupted")
            .expect("corrupt published object");
        assert!(matches!(
            publisher.publish(&archive, &packaged.source_digest, limits()),
            Err(CasArchiveError::DigestMismatch | CasArchiveError::UnsafeArchive)
        ));

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn publisher_never_commits_under_a_mismatched_digest() {
        let root = std::env::temp_dir().join(format!("rustok-source-cas-{}", Uuid::new_v4()));
        let source = root.join("source");
        let cas = root.join("cas");
        let archive = root.join("source.tar");
        fs::create_dir_all(&source).expect("source directory");
        fs::create_dir(&cas).expect("CAS directory");
        fs::write(source.join("source.txt"), b"immutable source\n").expect("source file");
        SourceArchiveBuilder::new(limits())
            .write(&source, &archive)
            .expect("source archive");
        let publisher = CasArchivePublisher::new(cas.clone()).expect("publisher");
        let mismatched = format!("sha256:{}", "f".repeat(64));

        assert_eq!(
            publisher.publish(&archive, &mismatched, limits()),
            Err(CasArchiveError::DigestMismatch)
        );
        assert_eq!(
            fs::read_dir(&cas).expect("CAS listing").count(),
            0,
            "a mismatched upload must leave no visible or temporary object"
        );

        fs::remove_dir_all(root).expect("remove test root");
    }
}
