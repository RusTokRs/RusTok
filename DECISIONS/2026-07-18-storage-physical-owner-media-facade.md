# Storage as the physical file owner and Media as the media facade

- Date: 2026-07-18
- Status: Accepted

## Context

RusToK has several consumers that need binary objects: media assets, AI
knowledge sources, module artifacts and future file-oriented capabilities. A
domain module must not become the owner of a storage backend or make other
modules depend on media-specific metadata.

## Decision

`rustok-storage` owns the physical object lifecycle and backend abstraction:

- storing, reading and deleting bytes;
- backend selection (local development or durable object storage);
- path safety, object URLs and content-addressed writes;
- backend-level size, content type and object metadata.

`rustok-media` is a domain facade and metadata index over storage for media
assets such as images, video and PDF files. It owns media classification,
translations, usage and media-specific upload/read/delete policy. Its
`storage_path` and `storage_driver` fields are references to storage objects,
not ownership of the binary backend.

Other consumers keep their own meaning and metadata while referencing storage
objects. In particular, `rustok-ai` owns knowledge sources, extracted text,
chunks, embeddings and citations; it does not duplicate the source binary.
`rustok-modules` owns artifact identity and release metadata while using the
same storage contract for artifact blobs.

Media discovery and management use the Media-owned metadata/index and typed
ports. Storage's trusted-prefix `list` operation remains an internal
reconciliation primitive, not a user-facing file browser.

## Consequences

- No module moves binary data into its own database or exposes raw storage
  handles through cross-module ports.
- A file can participate in more than one domain view without duplicating its
  bytes: for example, a PDF may be a Media asset and an AI knowledge source.
- New file-oriented domains must add their own metadata facade over the shared
  storage contract instead of extending `rustok-media` with unrelated policy.
- A future generic file registry is an additive contract decision; it is not
  required to change the current storage/media boundary.

## Related contracts

- [`rustok-storage`](../crates/rustok-storage/docs/README.md)
- [`rustok-media`](../crates/rustok-media/docs/README.md)
- [Neutral sandbox foundation](./2026-07-11-neutral-sandbox-foundation.md)
