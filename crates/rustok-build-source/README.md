# rustok-build-source

## Purpose

`rustok-build-source` owns the single hardened materialization contract for
digest-addressed source archives consumed by RusToK build workers.

## Responsibilities

- Resolve only exact `cas://sha256:<hex>` references from a fixed deployment
  root.
- Re-hash every archive before extraction.
- Enforce bounded strict USTAR entries, bytes, paths, types, checksums, duplicate
  rejection, and a complete zero-block terminator.
- Materialize only into a caller-provided new absolute directory.
- Remove a partially created destination after any extraction failure.

## Non-responsibilities

- It does not fetch from OCI, Git, HTTP, or arbitrary filesystem references.
- It does not own build policy, Cargo, credentials, publication, job leases, or
  runtime installation.
- It does not retain a second permissive archive path for compatibility.

## Interactions and entry points

- `CasArchiveStore` binds a canonical deployment root.
- `ArchiveLimits` supplies the caller-owned archive, extraction, and entry caps.
- `CasArchiveStore::materialize` returns only a digest and bounded-count receipt.
- See [local documentation](docs/README.md).
