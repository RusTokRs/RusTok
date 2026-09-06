# Documentation `rustok-media`

`rustok-media` is the domain owner and metadata index for media asset management on the platform. It handles images, video and PDF assets while calling the host-provided `object_store` runtime directly; `rustok-storage` only constructs that runtime and enforces canonical keys.

## Purpose

- publish the canonical runtime media contract for upload, list, delete, translation, and public image delivery scenarios;
- keep media metadata, classification, validation, byte delivery, and transport surfaces inside the module;
- provide a platform media capability without diluting domain logic across host or consumer layers.

## Scope

- `MediaService`, media entities/DTOs, and translation update normalization;
- REST upload/list/get/delete/translation handlers on a narrow `MediaHttpRuntime` with explicit DB/storage handles;
- typed cross-module image contract `MediaImageDescriptor`, delivery profile, and direct-public/proxy-required/not-addressable URL policy;
- `MediaAssetReadPort` and `MediaAssetWritePort` metadata/control contracts;
- `MediaPublicImageReadPort`, which returns one owner result containing the canonical `MediaItem` and the public descriptor selected by Media policy;
- `MediaPublicImageService`, which turns only storage-relative image descriptors into `/api/media/public/images/{id}/{checksum_sha256}` capability URLs;
- an unauthenticated Media-owned capability GET that derives tenant authority from `TenantContext`, verifies the active ready image blob and checksum, reads the object, and returns immutable bytes with ETag, content length/type, and `nosniff`;
- loopback-verified `rustok-media-transport` tonic adapters for the existing generic metadata/control operations. Binary bodies remain outside gRPC; parity for the new public-image capability remains an explicit future extraction gate;
- GraphQL and REST adapters of the module;
- upload validation by size/MIME policy and tenant isolation before accessing storage;
- module-owned admin UI package `rustok-media-admin`;
- observability signals for upload, delete, rendition, upload sessions, reconciliation, and storage health;
- owner-local lifecycle persistence and restart-safe reconciliation;
- immutable image renditions with bounded processing;
- the `media/asset` translation-target provider, including exact-locale
  aggregate coverage and a tenant-scoped change cursor;
- Local and env-gated S3-compatible lifecycle integration sources.

## Public image delivery contract

1. A consumer requests a public image descriptor through `MediaPublicImageReadPort` with a deadline-bound `PortContext`.
2. Media loads only the tenant-scoped active asset and ready active blob.
3. Media returns the canonical `MediaItem` so the consumer can apply its own owner relation rule, such as Profiles uploader validation.
4. Absolute/root-relative public URLs remain unchanged.
5. Storage-relative image paths are replaced inside Media with a capability URL containing the asset id and active blob SHA-256. Consumers do not see or reconstruct the object key.
6. Opaque or non-image references return no descriptor.
7. The capability GET repeats tenant, lifecycle, image MIME, checksum, and object-size validation before reading bytes.
8. Invalid id, checksum, tenant, lifecycle, state, or MIME combinations are exposed uniformly as not found. Storage/database failures use static unavailable responses and retain details only in logs.
9. The URL is immutable for one active blob and carries one-year immutable cache semantics. A new active blob receives a different checksum URL; a deleted/failed asset no longer resolves.

Direct-public media has the same public-delivery revocation model as before: once a public URL is disclosed, intermediary/client cache lifetime is governed by its immutable content identity. Consumers must not use this capability for private binary delivery.

## Integration

- uses the host-provided direct `object_store` runtime; Media rows keep immutable object references and lifecycle metadata;
- `apps/server` remains the composition root and wiring layer for media routes/GraphQL;
- runtime guard relies on tenant resolution for public image delivery and tenant-scoped module enablement for other public surfaces;
- Profiles consumes the owner descriptor and separately revalidates tenant, profile uploader, and image MIME before presentation;
- `rustok-seo` and other metadata consumers may emit only Media-approved public descriptors;
- no consumer reads Media tables, object keys, or storage handles directly;
- Translation consumes exact Media coverage only through
  `TranslationTargetProvider::read_progress`. Media aggregates source-eligible
  active assets, counts only exact target-row values, and brackets the
  aggregate with its owner cursor. Translation writes, translated-asset
  deletion, and active-asset failure all append cursor evidence transactionally;
- whole-module remote extraction must define how the Media-owned public URL and byte endpoint are reached before claiming provider parity. The current capability is embedded-runtime source-complete only.

## Verification

- `cargo test -p rustok-media --test public_image_proxy -- --nocapture`
- `node scripts/verify/verify-media-public-image-proxy.mjs`
- `cargo xtask module validate media`
- `cargo xtask module test media`
- `cargo test -p rustok-media-transport`
- existing targeted upload, translation, lifecycle, reconciliation, admin, Local, and S3 suites.

These commands are maintainer-run and were not executed while publishing this slice.

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Admin package](../admin/README.md)
- [gRPC transport](../../rustok-media-transport/docs/README.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
