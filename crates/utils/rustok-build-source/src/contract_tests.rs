use std::{
    fs,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{
    ArchiveLimits, CasArchiveError, CasArchivePublisher, CasArchiveStore, SourceArchiveBuilder,
    SourceArchiveError, SourceArchiveInspector, SourceTreeError, SourceTreeFile,
    SourceTreeMaterializer, CAS_SOURCE_URI_SCHEME, COPY_BUFFER_BYTES, USTAR_BLOCK_BYTES,
};

struct TestDir(PathBuf);

impl TestDir {
    fn new(prefix: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("rustok-contract-{prefix}-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create test directory");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn standard_limits() -> ArchiveLimits {
    ArchiveLimits::new(1024 * 1024, 2 * 1024 * 1024, 256).expect("standard limits")
}

#[test]
fn test_archive_limits_bounds_and_getters() {
    let limits = standard_limits();
    assert_eq!(limits.max_archive_bytes, 1024 * 1024);
    assert_eq!(limits.max_extracted_bytes, 2 * 1024 * 1024);
    assert_eq!(limits.max_entries, 256);

    // Zero bounds must be rejected
    assert_eq!(
        ArchiveLimits::new(0, 1024, 10),
        Err(CasArchiveError::ResourceLimit)
    );
    assert_eq!(
        ArchiveLimits::new(1024, 0, 10),
        Err(CasArchiveError::ResourceLimit)
    );
    assert_eq!(
        ArchiveLimits::new(1024, 1024, 0),
        Err(CasArchiveError::ResourceLimit)
    );

    // Clone, Copy, Debug, PartialEq, Eq
    let copy = limits;
    assert_eq!(limits, copy);
    let debug = format!("{limits:?}");
    assert!(debug.contains("max_archive_bytes"));
}

#[test]
fn test_constants_and_schemes() {
    assert_eq!(USTAR_BLOCK_BYTES, 512);
    assert_eq!(COPY_BUFFER_BYTES, 64 * 1024);
    assert_eq!(CAS_SOURCE_URI_SCHEME, "cas://");
}

#[test]
fn test_source_tree_file_contract() {
    let file = SourceTreeFile::new("src/main.rs", b"fn main() {}\n");
    assert_eq!(file.path, "src/main.rs");
    assert_eq!(file.contents, b"fn main() {}\n");

    let cloned = file.clone();
    assert_eq!(file, cloned);
    let debug = format!("{file:?}");
    assert!(debug.contains("src/main.rs"));
}

#[test]
fn test_source_archive_builder_and_materializer_limits_getters() {
    let limits = standard_limits();
    let builder = SourceArchiveBuilder::new(limits);
    assert_eq!(builder.limits(), limits);

    let materializer = SourceTreeMaterializer::new(limits);
    assert_eq!(materializer.limits(), limits);

    let inspector = SourceArchiveInspector::new(limits);
    assert_eq!(inspector.limits(), limits);
}

#[test]
fn test_source_tree_materializer_in_memory_validation() {
    let materializer = SourceTreeMaterializer::new(standard_limits());

    // Valid file list
    let files = vec![
        SourceTreeFile::new("Cargo.toml", b"[package]\nname = \"pkg\"\nversion = \"0.1.0\"\n"),
        SourceTreeFile::new("src/lib.rs", b"pub fn answer() -> u32 { 42 }\n"),
    ];
    let validation = materializer.validate(&files).expect("validation should succeed");
    assert_eq!(validation.entries, 3); // 2 files + 1 intermediate directory "src"
    assert!(validation.source_bytes > 0);

    // Empty list is rejected
    assert_eq!(materializer.validate(&[]), Err(SourceTreeError::UnsafeSource));

    // Duplicate path is rejected
    let duplicates = vec![
        SourceTreeFile::new("src/lib.rs", b"// first"),
        SourceTreeFile::new("src/lib.rs", b"// second"),
    ];
    assert_eq!(
        materializer.validate(&duplicates),
        Err(SourceTreeError::UnsafeSource)
    );

    // Backslash path is rejected
    let backslash = vec![SourceTreeFile::new("src\\lib.rs", b"// backslash")];
    assert_eq!(
        materializer.validate(&backslash),
        Err(SourceTreeError::UnsafeSource)
    );

    // Path traversal is rejected
    let traversal = vec![SourceTreeFile::new("../escape.rs", b"// escape")];
    assert_eq!(
        materializer.validate(&traversal),
        Err(SourceTreeError::UnsafeSource)
    );

    // Forbidden paths are rejected
    let cargo_config = vec![SourceTreeFile::new(".cargo/config.toml", b"")];
    assert_eq!(
        materializer.validate(&cargo_config),
        Err(SourceTreeError::UnsafeSource)
    );

    let descriptor = vec![SourceTreeFile::new("module-artifact-descriptor.json", b"{}")];
    assert_eq!(
        materializer.validate(&descriptor),
        Err(SourceTreeError::UnsafeSource)
    );
}

#[test]
fn test_source_tree_materializer_filesystem_write() {
    let root = TestDir::new("mat-write");
    let dest = root.path().join("materialized_src");
    let materializer = SourceTreeMaterializer::new(standard_limits());

    let files = vec![
        SourceTreeFile::new("Cargo.toml", b"[package]\nname = \"hello\"\n"),
        SourceTreeFile::new("nested/deep/module.rs", b"pub fn deep() {}\n"),
    ];

    let result = materializer.write(&files, &dest).expect("write tree");
    assert_eq!(result.entries, 4); // 2 files + 2 directories (nested, nested/deep)
    assert!(dest.join("Cargo.toml").is_file());
    assert!(dest.join("nested/deep/module.rs").is_file());

    // Writing to existing destination must fail
    assert_eq!(
        materializer.write(&files, &dest),
        Err(SourceTreeError::DestinationExists)
    );
}

#[test]
fn test_source_archive_builder_and_cas_pipeline() {
    let test_dir = TestDir::new("cas-pipeline");
    let src_dir = test_dir.path().join("source");
    let cas_dir = test_dir.path().join("cas");
    let archive_path = test_dir.path().join("archive.tar");
    let target_dir = test_dir.path().join("extracted");

    fs::create_dir_all(&cas_dir).expect("cas dir");

    // 1. Materialize source
    let materializer = SourceTreeMaterializer::new(standard_limits());
    let files = vec![
        SourceTreeFile::new("Cargo.toml", b"[package]\nname = \"pkg\"\nversion = \"0.1.0\"\n"),
        SourceTreeFile::new("src/lib.rs", b"pub fn code() -> bool { true }\n"),
    ];
    materializer.write(&files, &src_dir).expect("write tree");

    // 2. Build archive
    let builder = SourceArchiveBuilder::new(standard_limits());
    let receipt = builder.write(&src_dir, &archive_path).expect("write archive");
    assert!(receipt.source_digest.starts_with("sha256:"));
    assert!(receipt.archive_bytes > 0);
    assert_eq!(receipt.entries, 2);

    // 3. Inspect archive
    let inspector = SourceArchiveInspector::new(standard_limits());
    let inspection = inspector.inspect(&archive_path).expect("inspect archive");
    assert_eq!(inspection.source_digest, receipt.source_digest);
    assert_eq!(inspection.archive_bytes, receipt.archive_bytes);
    assert_eq!(inspection.entries, receipt.entries);

    // 4. Publish to CAS
    let publisher = CasArchivePublisher::new(cas_dir.clone()).expect("cas publisher");
    assert_eq!(publisher.root(), cas_dir.canonicalize().unwrap().as_path());

    let pub_receipt1 = publisher
        .publish(&archive_path, &receipt.source_digest, standard_limits())
        .expect("publish once");
    assert!(pub_receipt1.created);
    assert_eq!(pub_receipt1.source_digest, receipt.source_digest);

    // Re-publish is idempotent (created: false)
    let pub_receipt2 = publisher
        .publish(&archive_path, &receipt.source_digest, standard_limits())
        .expect("publish twice");
    assert!(!pub_receipt2.created);
    assert_eq!(pub_receipt2.source_digest, receipt.source_digest);

    // 5. Materialize from CAS via Store
    let store = CasArchiveStore::new(cas_dir.clone()).expect("cas store");
    assert_eq!(store.root(), cas_dir.canonicalize().unwrap().as_path());

    let uri = format!("cas://{}", receipt.source_digest);
    let mat_receipt = store
        .materialize(&uri, &receipt.source_digest, &target_dir, standard_limits())
        .expect("materialize from cas");
    assert_eq!(mat_receipt.source_digest, receipt.source_digest);
    assert!(target_dir.join("Cargo.toml").is_file());
    assert!(target_dir.join("src/lib.rs").is_file());

    // 6. Equality checks
    assert_eq!(publisher, publisher.clone());
    assert_eq!(store, store.clone());
}

#[test]
fn test_destination_collision_and_digest_mismatch() {
    let test_dir = TestDir::new("edge-cases");
    let cas_dir = test_dir.path().join("cas");
    fs::create_dir_all(&cas_dir).expect("cas dir");

    let publisher = CasArchivePublisher::new(cas_dir.clone()).expect("publisher");
    let fake_archive = test_dir.path().join("empty.tar");
    fs::write(&fake_archive, b"invalid archive bytes").expect("write fake");

    // Invalid archive structure fails with UnsafeArchive
    let mismatch_digest = format!("sha256:{}", "0".repeat(64));
    let err = publisher.publish(&fake_archive, &mismatch_digest, standard_limits());
    assert!(matches!(
        err,
        Err(CasArchiveError::UnsafeArchive | CasArchiveError::DigestMismatch)
    ));

    // Store rejected for non-absolute root
    assert!(CasArchiveStore::new(PathBuf::from("relative/path")).is_err());
    assert!(CasArchivePublisher::new(PathBuf::from("relative/path")).is_err());

    // Builder rejected for relative source
    let builder = SourceArchiveBuilder::new(standard_limits());
    assert_eq!(
        builder.write(Path::new("relative/src"), Path::new("/tmp/out.tar")),
        Err(SourceArchiveError::InvalidSource)
    );
}
