# rustok-build-source

## Purpose

`rustok-build-source` owns deterministic construction, read-only inspection,
and hardened materialization for the archive source media type. The current
crate also contains the archive-specific deployment CAS writer; the accepted
release-safety cutover below removes that second layout authority.

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
- In the current implementation, publish a verified archive into the deployment
  source CAS through a private copy, second streaming digest check, and atomic
  no-replace commit.
- Inspect a standalone source archive through the same parser used by workers,
  including hashing it without materializing files.

## Non-responsibilities

- It does not fetch from OCI, Git, HTTP, or arbitrary filesystem references.
- It does not own build policy, Cargo, credentials, publication, job leases, or
  runtime installation.
- It does not retain a second permissive archive path for compatibility.

## Interactions and entry points

- `CasArchiveStore` binds a canonical deployment root.
- `CasArchivePublisher` is the current archive-specific control-plane writer;
  it is a named cutover gap, not the target source-CAS owner.
- `ArchiveLimits` supplies the caller-owned archive, extraction, and entry caps.
- `CasArchiveStore::materialize` returns only a digest and bounded-count receipt.
- `SourceArchiveBuilder::write` produces the canonical local source archive and
  its immutable digest receipt.
- `SourceArchiveInspector::inspect` validates and hashes a standalone archive
  without extracting it.
- See [local documentation](docs/README.md).

## Accepted release-safety cutover

The canonical source CAS is media-type neutral: the `rustok-modules`
preparation owner publishes a globally deduplicated `source_digest` blob plus a
distinct owner/RLS-scoped `source_receipt_id` over owner/preparation, media
type, length, and manifest through its single
`SourceObjectStore` port. This crate remains the archive builder, parser, inspector,
materializer, and archive-specialized client of that owner. The direct
`<digest>.tar` writer/layout is removed atomically when every repository caller
moves to the generic owner; no dual writer or compatibility lookup remains.
Reviewed Rhai releases use canonical bounded-workspace objects through the
generic owner and never pass through this archive or acquire a fake `.tar`
identity.
