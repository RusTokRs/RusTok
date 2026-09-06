# Implementation plan for `rustok-media`

## Target state

Media owns the complete asset lifecycle: stable assets, immutable source blobs, immutable renditions, edit recipes, upload sessions, translations, delivery descriptors, public image delivery, reconciliation, and deletion evidence. Bytes live in the direct `object_store` runtime. Searchable ownership and lifecycle state live in Media-owned database tables.

The modular monolith uses those tables in the shared PostgreSQL deployment. Whole-module extraction moves the schema and Media storage credentials together; it does not require a separate database server before extraction.

## Current state

- `rustok-media` publishes the migration for `media_assets`, `media_blobs`,
  `media_renditions`, `media_upload_sessions`, `media_translations`, and
  `media_translation_changes`. The Core Outbox migration owns the shared
  `owner_operation_receipts` ledger used by Media under owner slug `media`.
- Media translation writes convert transport strings to the canonical
  `rustok_api::TenantLocale` at the owner boundary; `und` and malformed locale
  tags cannot reach `media_translations`.
- Exact translation apply uses durable locale revisions and one transaction
  over the active asset plus ordered source/target rows. Stale source, stale
  target, unexpected target creation, and revision exhaustion fail closed.
- `MediaTranslationTargetProvider` is registered by the server composition
  root as `media/asset`. It exposes bounded UUID cursor discovery, exact
  locale-only snapshots, stable field identities and hashes, permission floors,
  validation, one-resource CAS apply, exact source/target aggregate coverage,
  typed error severity, an explicit empty protected-token ledger for its
  plain-text fields, and a tenant-scoped owner change cursor. Aggregate reads
  use a stable
  before/after cursor window, count no locale fallback, and expose only
  source-eligible active assets.
- Provider apply commits the exact target update, stable idempotency receipt,
  append-only change record, and content-free `translation.target.changed`
  outbox event in the same transaction. Replay returns the original receipt
  without another write, cursor record, or event.
- Provider authorization is evaluated on every apply call. Its durable
  idempotency request hash is actor-neutral and binds the tenant, operation
  kind, exact patch, and mutation key, so a separately authorized Translation
  recovery operator can reconcile the original mutation identity without
  duplicating the owner write.
- Non-provider Media translation writes commit the same append-only change
  record and owner event atomically with the locale row, so REST, GraphQL,
  native, AI-originated, embedded-port, and provider paths cannot silently
  bypass inventory repair evidence.
- Translated-asset deletion and active-asset failure append deleted/unavailable
  cursor records and owner events in the same owner transaction as their
  lifecycle transition, preventing false-current Translation projections.
- Production inventory enablement still requires projection replay, cursor
  checkpoint recovery, and sustained multi-replica operational evidence.
- Direct uploads write an object, then atomically persist one asset and one immutable ready blob. Ambiguous database outcomes preserve the object for reconciliation.
- Blob rows persist SHA-256, verified MIME, size, dimensions, timestamps, lifecycle state, retry count, and last error.
- Delete requests persist tombstones before deleting objects. Successful deletion and `NotFound` complete tombstones; transient failures remain restart-safe work.
- Reconciliation prioritizes delete-pending rows, rotates ready rows through persisted progress, isolates missing rendition results, expires upload sessions, and removes only eligible staging objects.
- Presigned S3 sessions persist tenant/actor, expected type/size, staging key, expiry, and completion.
- Immutable recipes normalize orientation and apply bounded transforms/encoders through a bounded worker.
- Media descriptors classify URLs as direct-public, proxy-required, or not-addressable.
- `MediaPublicImageReadPort` returns one owner result containing the canonical `MediaItem` and the descriptor selected by Media delivery policy.
- `MediaPublicImageService` preserves direct-public descriptors, replaces storage-relative image paths with `/api/media/public/images/{id}/{active_blob_sha256}`, and drops opaque references.
- The public image handler verifies tenant, active asset/blob, ready state, image MIME, checksum, object metadata size, and body size. It returns checksum ETag, one-year immutable cache semantics, content type/length, and `nosniff`.
- Invalid id/checksum/tenant/lifecycle/state/MIME combinations are indistinguishable as not found; storage/database/invariant failures return static unavailable messages while details remain in logs.
- Profiles consumes this owner surface for GraphQL and native storefront avatar/banner presentation and independently revalidates tenant, uploader, and image MIME.
- `rustok-media-transport` now carries `MediaPublicImageAsset` through `GetPublicImageAsset` with deadline propagation, exact typed owner errors, trusted authority, and explicit operation authorization. Image bytes remain outside gRPC.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Provider contracts: `MediaAssetReadPort`, `MediaPublicImageReadPort`, and `MediaAssetWritePort`.
- Cross-module consumers receive typed descriptors and control operations, never object-store handles or storage keys.
- Local streaming REST and presigned object-store PUT are Media-owned binary transports. The public image capability GET is also Media-owned binary delivery; generic gRPC DTOs carry no bytes.
- `rustok-media-transport` provides tonic client/server adapters for metadata, public descriptor selection, and write/control contracts.
- The public-image gRPC server requires explicit `with_public_image_provider(...)` configuration and a separate `MediaGrpcOperation::GetPublicImageAsset` grant. Generic asset-read authorization does not imply public URL selection.
- Source-level embedded/loopback conformance now covers owner-selected capability descriptors, deadlines, deleted-state errors, and explicit trusted operation grants.
- Remote byte delivery is not a gRPC concern. Extracted deployments still need a reachable Media HTTP ingress, public base routing when a shared relative ingress is unavailable, cache/conditional-GET evidence, mTLS, readiness, isolated database/storage, rollback, and performance evidence.
- This addition does not promote Media beyond its current FBA status.

## Public image capability model

```text
/api/media/public/images/{asset_id}/{active_blob_sha256}
```

- The asset id is stable; the checksum binds the URL to one immutable active blob.
- Changing the active blob changes the capability URL.
- Deleting/failing the asset or active blob invalidates resolution.
- The URL contains no object key, tenant id, uploader id, filename, or storage backend detail.
- Tenant is selected by host `TenantContext`, not accepted from the path.
- The handler is unauthenticated because it serves an already approved public descriptor. It is not a private-download authorization mechanism.
- Capability disclosure has the same cache/revocation model as direct-public immutable media. Consumers must not use this path for private binary content.
- A relative capability URL requires deployment routing that sends the path to the Media HTTP owner. A separate Media origin must publish an owner-configured public URL before production extraction.

## Object layout

Media uses only `ObjectKey::chronological`:

```text
media/objects/tenants/{tenant_id}/YYYY/MM/DD/{shard}/{blob_id}.{ext}
media/staging/tenants/{tenant_id}/YYYY/MM/DD/{shard}/{upload_id}.upload
```

The database remains the index; consumers never derive or list these keys.

## Delivery order

1. **Completed — direct storage and canonical keys.**
2. **Completed — Media-owned persistence and lifecycle evidence.**
3. **Implemented — immutable image renditions and bounded processing.**
4. **Implemented — restart-safe reconciliation.**
5. **Implemented — metadata/write extraction control-plane conformance; rerun evidence after lifecycle/receipt changes.**
6. **Source-complete — public image capability and remote descriptor control plane.** Media owns descriptor selection and HTTP byte delivery; embedded and loopback gRPC providers expose the same `MediaPublicImageAsset`; Profiles consumes the owner result without constructing URLs. Compiled/runtime Local/S3, HTTP cache/conditional GET, deployment ingress, degradation, and production remote-provider evidence remain pending.

## Verification

- `scripts/verify/verify-media-admin-boundary.mjs` is the fast boundary
  guardrail for the module-owned admin package. It locks the host-neutral
  `HostRuntimeContext` native adapter alongside the parallel GraphQL and REST
  adapters.
- `contracts/media-fba-registry.json` is the machine-readable provider
  contract. Its static and degraded-path evidence is retained in
  `contracts/evidence/media-contract-test-static-matrix.json`,
  `contracts/evidence/media-runtime-fallback-smoke.json`, and
  `contracts/evidence/media-port-error-matrix.json`.
- The whole-module extraction pilot and public URL policy remain governed by
  `DECISIONS/2026-07-16-media-search-extraction-boundaries.md`;
  `MediaAssetSummary` is the content-free read projection used by the FBA
  contract.
- `cargo test -p rustok-media --test public_image_proxy -- --nocapture`
- `cargo test -p rustok-media-transport --test port_conformance -- --nocapture`
- `node scripts/verify/verify-media-public-image-proxy.mjs`
- `cargo test -p rustok-media`
- `cargo test -p rustok-media-transport`
- `cargo test -p rustok-media --features s3 --test s3_lifecycle`
- `cargo xtask module validate media`
- `cargo xtask module test media`
- `npm run verify:media:admin-boundary`
- `npm run verify:media:fba`
- `cargo test -p rustok-storage --all-features`

These commands are maintainer-run and were not executed while publishing this slice. Required new evidence covers storage-relative descriptor issuance, direct-public preservation, wrong-checksum/cross-tenant masking, active/ready/image gates, object-body delivery, immutable cache/ETag, Profiles owner revalidation, embedded/loopback descriptor parity, and production Media HTTP reachability.

## Change rules

1. Media owns media metadata, lifecycle, public descriptor selection, and public byte delivery; `rustok-storage` owns none of those domain decisions.
2. Never mutate an original or rendition object in place.
3. Never query media by listing object-store folders.
4. Never expose object keys through public capability paths or consumer DTOs.
5. Consumers may apply their own relation policy to the returned `MediaItem`, but may not reconstruct a public URL.
6. Never add image bytes to generic gRPC DTOs; extracted deployments route the returned descriptor to Media-owned HTTP delivery.
7. Do not claim production public-image remote parity until the extracted deployment owns a reachable public URL/byte endpoint with retained cache, authority, readiness, and rollback evidence.
8. Keep FFA/FBA status and central registry evidence synchronized with UI or transport-boundary changes.
