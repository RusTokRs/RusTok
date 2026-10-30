# `rustok-build-source` Crate API & Architecture Contract

## 1. Architectural Purpose & Boundaries

`rustok-build-source` defines the deterministic, content-addressable storage (CAS) and source packaging boundary for RusToK module builds. It ensures that source code trees are safely validated in memory, deterministically serialized into POSIX USTAR archives without non-deterministic metadata, verified against cryptographic SHA-256 digests, and materialized into isolated build workspaces without directory escapes or symlink attacks.

Key responsibilities:
- In-memory validation and filesystem materialization of reviewed source trees (`SourceTreeMaterializer`, `SourceTreeFile`).
- Deterministic creation of POSIX USTAR tar archives (`SourceArchiveBuilder`) with normalized permissions and zeroed timestamps.
- Inspection and verification of archive contents against strict bounds (`SourceArchiveInspector`, `ArchiveLimits`).
- Content-addressable storage operations (`CasArchiveStore`, `CasArchivePublisher`) providing atomic no-replace commits via filesystem hard links.
- Prevention of sandbox escapes, path traversals (`..`), forbidden files (`.cargo/config.toml`, `module-artifact-descriptor.json`), and non-regular filesystem entries.

## 2. Deterministic POSIX USTAR Archive Serialization & Anti-Escape Invariants

To guarantee bit-for-bit reproducible source digests across different build hosts and times:
1. **Lexicographical Entry Ordering**:
   - Files are sorted lexicographically by normalized POSIX path prior to archive serialization.
2. **Normalized POSIX Header Fields**:
   - File modes are forced to octal `0644`.
   - UID, GID, and modification timestamps are normalized to `0`.
   - Path fields adhere strictly to POSIX USTAR specifications (prefix up to 155 bytes, name up to 100 bytes).
3. **Anti-Escape Invariants**:
   - Rejects directory traversal components (`..`), root paths, and non-normal path components.
   - Forbids symlinks, device nodes, FIFOs, and non-regular files.
   - Strictly blocks `.cargo/config`, `.cargo/config.toml`, and `module-artifact-descriptor.json` inside packaged source trees to prevent build sandbox escapes.
   - Validates octal checksums and trailing zero blocks; rejects non-zero padding bytes.

## 3. In-Memory Source Tree Materialization & Validation

`SourceTreeMaterializer`:
- `validate(&self, files: &[SourceTreeFile]) -> Result<SourceTreeMaterialization, SourceTreeError>`:
  Validates incoming in-memory reviewed source trees without touching the host filesystem. Enforces unique normalized paths, max extracted byte limits, and max entry count limits.
- `write(&self, files: &[SourceTreeFile], destination: &Path) -> Result<SourceTreeMaterialization, SourceTreeError>`:
  Atomically writes the reviewed source into a brand-new directory under a regular parent directory. On any failure, cleans up only the created directory and never caller-owned paths.

## 4. Content-Addressable Storage (CAS) Store & Atomic Publisher Pipeline

`CasArchivePublisher` and `CasArchiveStore`:
- **Atomic No-Replace Publish**:
  Uploads source archives into a temporary file on the same filesystem and establishes the final CAS object (`<sha256>.tar` or direct digest path) via `fs::hard_link`. If the target already exists, the publisher validates the existing content and returns `created: false` idempotently without overwriting.
- **Identity Pinning**:
  Validates `cas://<sha256>` URIs and rejects any digest mismatch before committing to CAS.
- **Safe Workspace Extraction**:
  `CasArchiveStore::materialize` unpacks the CAS archive into an isolated destination directory with strict boundary checks, ensuring extracted files remain within the destination root.

## 5. Universal Logic Dimensions Verification

1. **Preconditions & Bounds**:
   - Archive and extraction bounds enforced by `ArchiveLimits` (`max_archive_bytes`, `max_extracted_bytes`, `max_entries`).
   - Zero-valued bounds are rejected at creation.
   - Maximum entry count capped at `MAX_ARCHIVE_ENTRIES` (65,536).
2. **Branch Completeness & Anti-Swallowing**:
   - All I/O, format, and limit errors mapped to typed domain errors (`CasArchiveError`, `SourceArchiveError`, `SourceTreeError`).
   - Non-zero padding, corrupted headers, and truncated archives trigger explicit `UnsafeArchive` or `CorruptedArchive`.
3. **State Transition Integrity**:
   - Materialization and publishing operations clean up temporary files and directories on any error, preventing partial or corrupt artifacts from lingering in CAS or workspaces.
4. **Side-Effect Completeness**:
   - Files created during materialization are explicitly flushed and synced (`sync_all()`).
   - Atomic CAS publication uses hard-links ensuring atomic file system visibility.
5. **Algorithmic & Calculation Precision**:
   - Strict POSIX USTAR 8-byte octal parsing and checksum verification.
   - Streaming SHA-256 digest calculation matches exact archive byte sequences.
6. **Order of Operations & Concurrency**:
   - Safe destination checks performed before creating files.
   - Atomic hard-link CAS publication guarantees race-free concurrent uploads without data loss or corruption.
