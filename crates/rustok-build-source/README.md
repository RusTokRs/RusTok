# rustok-build-source

## Purpose

`rustok-build-source` owns the single deterministic write, read-only inspection,
and hardened materialization contract for digest-addressed source archives.

## Responsibilities

- Resolve only exact `cas://sha256:<hex>` references from a fixed deployment
  root.
- Re-hash every archive before extraction.
- Enforce bounded strict USTAR entries, bytes, paths, types, checksums, duplicate
  rejection, and a complete zero-block terminator.
- Materialize only into a caller-provided new absolute directory.
- Remove a partially created destination after any extraction failure.
- Write file-only deterministic USTAR archives with sorted paths, fixed metadata,
  zero padding, bounded bytes/entries, and create-new destination semantics.
- Publish a verified archive into the deployment source CAS through a private
  copy, second streaming digest check, and atomic no-replace commit.
- Inspect a standalone source archive through the same parser used by workers,
  including hashing it without materializing files.

## Non-responsibilities

- It does not fetch from OCI, Git, HTTP, or arbitrary filesystem references.
- It does not own build policy, Cargo, credentials, publication, job leases, or
  runtime installation.
- It does not retain a second permissive archive path for compatibility.

## Interactions and entry points

- `CasArchiveStore` binds a canonical deployment root.
- `CasArchivePublisher` owns atomic control-plane writes to that root.
- `ArchiveLimits` supplies the caller-owned archive, extraction, and entry caps.
- `CasArchiveStore::materialize` returns only a digest and bounded-count receipt.
- `SourceArchiveBuilder::write` produces the canonical local source archive and
  its immutable digest receipt.
- `SourceArchiveInspector::inspect` validates and hashes a standalone archive
  without extracting it.
- See [local documentation](docs/README.md).
