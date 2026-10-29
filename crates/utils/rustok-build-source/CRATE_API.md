# rustok-build-source / CRATE_API

## Public surface

### Primary Store & Limits
- `CasArchiveStore`: content-addressable storage writer and reader for deterministic immutable source archives.
- `ArchiveLimits`: bounds descriptor (`max_archive_bytes`, `max_extracted_bytes`, `max_entries`).
- `CasArchiveError`: typed error enum for CAS operations (`ResourceLimit`, `Io`, `CorruptedArchive`, `MismatchedDigest`).

### Source Tree & Archive Builders
- `SourceArchiveBuilder`: creates deterministic, ustar-formatted tar archives with normalized entry permissions and timestamps.
- `SourceArchiveReceipt`: confirmation of generated archive containing exact byte count and SHA-256 digest.
- `SourceTreeMaterializer`: safely unpacks archives into isolated workspace directories without directory traversal (`..`) or symlink breakouts.
- `SourceTreeFile`: descriptor of individual file entries within a materialized source tree.

## Invariants

- **Safe Code Boundary**: Forbids unsafe code (`#![forbid(unsafe_code)]`).
- **Deterministic Serialization**: Archive entries are sorted lexicographically, timestamps and file modes are normalized, eliminating non-deterministic archive digests.
- **Path Traversal Protection**: Archive extraction strictly rejects relative directory escapes (`..`), absolute paths, device nodes, and symlinks pointing outside the extraction root.
- **Content-Addressable Immutability**: Artifacts stored in CAS are identified solely by their SHA-256 digest and cannot be modified after commit.
