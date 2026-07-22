# Implementation plan for `rustok-media`

## Target state

Media owns the complete asset lifecycle: stable assets, immutable source blobs,
immutable renditions, edit recipes, upload sessions, translations, delivery
descriptors, reconciliation, and deletion evidence. Bytes live in the direct
`object_store` runtime. Searchable ownership and lifecycle state live in
Media-owned database tables.

The modular monolith uses those tables in the shared PostgreSQL deployment.
Whole-module extraction moves the schema and Media storage credentials together;
it does not require a separate database server before extraction.

## Current state

- `rustok-media` publishes the migration for `media_assets`, `media_blobs`,
  `media_renditions`, `media_upload_sessions`, `media_translations`, and
  `media_port_operations`.
- Direct uploads write an object, then atomically persist one asset and one
  immutable ready blob. A database error is verified against the durable row
  before compensation; ambiguous commit outcomes preserve the object for
  reconciliation instead of risking a dangling DB reference.
- Blob rows persist SHA-256, verified MIME, size, dimensions, timestamps,
  lifecycle state, retry count, and last error. Backend kind is runtime
  diagnostics and is never domain data.
- Delete requests persist tombstones before deleting objects. Successful deletion
  and `NotFound` complete tombstones; transient failures remain restart-safe work.
- Reconciliation prioritizes delete-pending rows, rotates ready rows through a
  persisted `last_reconciled_at`, marks only a missing active source as an
  asset failure, isolates missing rendition results, completes delete-pending
  assets in a separate sweep, expires upload sessions, and removes only
  completed or expired staging objects.
- Presigned S3 sessions persist tenant/actor, expected type/size, staging key,
  expiry, and completion. Finalization checks object metadata before reading
  bytes and is idempotent through the unique asset `upload_session_id`.
- Immutable recipes normalize EXIF orientation and apply crop, quarter-turn,
  flips, and SIMD resize. JPEG, PNG, WebP, and AVIF outputs use production
  encoders with explicit alpha handling and stripped metadata.
- CPU work runs in a bounded `spawn_blocking` worker with at most two production
  slots. Input/output bytes, decoded memory, dimensions, pixels, frames,
  concurrency, and caller wait time are bounded. Native codec work retains its
  semaphore permit until it exits even if the caller deadline expires. Animated
  input is rejected rather than truncated.
- Golden output, resource-limit, concurrency, Local lifecycle, and live
  MinIO-compatible lifecycle/conformance tests are the verification gate for
  this implementation.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Provider contracts: `MediaAssetReadPort` and `MediaAssetWritePort`.
- Cross-module consumers receive typed descriptors and control operations,
  never raw object-store handles or binary bodies.
- Local streaming REST and presigned object-store PUT are Media-owned binary
  transports. Generic ports carry only metadata and completion control.
- `rustok-media-transport` provides the loopback-verified tonic client/server
  adapters. One compiled suite runs every read/write port operation against
  both the embedded provider and a real loopback gRPC provider.
- The modular monolith keeps the embedded provider as its default. A production
  process split requires deployment-owned mTLS, an interceptor that injects
  `TrustedMediaAuthority`, readiness, isolated database/storage, rollback, and
  performance evidence before
  `transport_verified` promotion.
- Static boundary evidence remains in `media-fba-registry.json`,
  `media-contract-test-static-matrix.json`, `media-runtime-fallback-smoke.json`,
  and `media-port-error-matrix.json`. `MediaAssetSummary` and the public URL policy
  remain the consumer-safe read model.
- This is the whole-module extraction pilot described by
  `2026-07-16-media-search-extraction-boundaries.md`.
- Native admin transport receives database and `StorageRuntime` through the
  host-neutral `HostRuntimeContext`. The fast boundary guard is
  `scripts/verify/verify-media-admin-boundary.mjs`.

## Object layout

Media uses only `ObjectKey::chronological`:

```text
media/objects/tenants/{tenant_id}/YYYY/MM/DD/{shard}/{blob_id}.{ext}
media/staging/tenants/{tenant_id}/YYYY/MM/DD/{shard}/{upload_id}.upload
```

- UTC creation date is immutable.
- The UUID-derived shard bounds local directory fan-out.
- The database is the index; object-store folder listing is never a media query.
- Original filenames, titles, locales, mutable slugs, backend names, and layout
  versions never enter keys.
- Extensions come from verified content or a controlled rendition format.

## Persistence model

- `media_assets`: stable tenant identity, uploader, optional upload-session
  identity, active source blob, metadata, and lifecycle timestamps.
- `media_blobs`: immutable object key, MIME, size, dimensions, checksum, state,
  retry evidence, and deletion timestamps.
- `media_renditions`: source blob, canonical recipe JSON/hash, result blob,
  purpose, state, and failure evidence; unique on source plus recipe hash.
- `media_upload_sessions`: original name, bounded staging key, expected type/size,
  actor, expiry, finalization, staging cleanup, and failure evidence.
- `media_translations`: localized title, alt text, and caption owned by the asset.

No other module creates, alters, or queries these tables directly.

## Image pipeline

The default pipeline is pure Rust:

- `image` for bounded decoding, EXIF orientation, transforms, PNG framing, and
  the AVIF encoder backed by `ravif`;
- `fast_image_resize` for SIMD resize and cover/contain behavior;
- `mozjpeg-rs` for progressive JPEG;
- `zenwebp` for WebP;
- `oxipng` for deterministic lossless PNG optimization.

`imageproc` is added only when a required editor operation is absent from
`image`. `libvips` remains an evidence-driven fallback if representative
benchmarks prove that the pure-Rust pipeline misses an accepted CPU or memory
budget.

## Delivery order

1. **Completed — direct storage and canonical keys.** All owners call
   `ObjectStore` directly and use the one chronological or digest key policy.
2. **Completed — Media-owned persistence.** Assets, blobs, renditions, upload
   sessions, and translations have explicit lifecycle evidence and migrations.
3. **Implemented — immutable image renditions.** Recipe hashing, normalized
   transforms, production encoders, golden fixtures, limits, worker execution,
   persistence, and idempotency are verified.
4. **Implemented — reconciliation instead of destructive cleanup.** Tombstones,
   missing-object failure state, staging expiry, compensation, and retry evidence
   are designed to be restart-safe and observable; the updated verification gate
   remains pending.
5. **Implemented — extraction conformance.** Direct and presigned Local/S3
   delivery are covered by the owner contracts. `rustok-media-transport` preserves
   deadlines and typed owner errors, keeps binary bodies outside gRPC, and has one
   compiled read/write port suite against embedded and loopback providers; rerun
   it after the lifecycle and receipt changes before transport promotion.

## Verification

- `cargo test -p rustok-media`
- `cargo test -p rustok-media-transport`
- `cargo test -p rustok-media --features s3 --test s3_lifecycle`
- `cargo xtask module validate media`
- `cargo xtask module test media`
- `npm run verify:media:admin-boundary`
- `npm run verify:media:fba`
- `cargo test -p rustok-storage --all-features`

The S3 suites run when `RUSTOK_TEST_S3_ENDPOINT` and matching bucket/credentials
are present. Required evidence covers direct upload, presigned PUT/finalize,
rendition, deletion, conditional create, prefix listing, multipart abort,
signing, compensation, reconciliation, and restart/idempotency behavior.

## Change rules

1. Media owns media metadata and lifecycle; `rustok-storage` owns neither.
2. Never mutate an original or rendition object in place.
3. Never query media by listing object-store folders.
4. Keep FFA/FBA status and central registry evidence synchronized with UI or
   transport-boundary changes.
