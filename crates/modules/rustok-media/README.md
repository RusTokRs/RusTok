# rustok-media

## Purpose

`rustok-media` owns media asset uploads, metadata, translations, delivery descriptors, and transport adapters for RusToK.

## Responsibilities

- Provide the shared media domain service and SeaORM entities for uploads and localized metadata with normalized locale/text translation inputs.
- Own media GraphQL and REST transport adapters for module-facing APIs.
- Keep REST upload/list/get/delete/translation handlers on narrow `MediaHttpRuntime` state; the manifest-declared Axum router builds it from `HostRuntimeContext` and a typed storage handle.
- Publish the module-owned Leptos admin UI crate `rustok-media-admin`.
- Own storage-backed media lifecycle state while calling the shared `object_store` runtime directly.
- Own write-port idempotency semantics and tenant-composite persistence
  integrity through the shared Outbox `owner_operation_receipts` ledger under
  the `media` owner namespace.
- Publish `MediaTranslationTargetProvider` for bounded exact-locale discovery,
  exact reads, validation, CAS apply, exact aggregate coverage, and
  tenant-scoped change-cursor repair through the shared translation target
  registry. Media metadata fields are plain text and therefore publish an
  explicit empty protected-token ledger.
- Commit every translation write with an append-only owner cursor record and
  content-free `translation.target.changed` outbox event in one owner
  transaction; provider apply also commits its stable idempotency receipt in
  that transaction.
- Commit translated-asset deletion and active-asset failure with
  deleted/unavailable translation cursor evidence so aggregate freshness
  cannot remain falsely current.
- Generate immutable source and rendition keys through the canonical tenant/date/shard policy.
- Own validated image edit recipes and bounded pure-Rust processing.
- Publish the module-local `rustok-media-cli` adapter with `media reconcile`, keeping CLI/runtime assembly outside the domain crate.
- Expose `MediaImageDescriptor` as the typed cross-module image contract (`url/alt/size/mime` + derived helpers, delivery profile, and public URL policy) for SEO and other read-side consumers.
- Publish `MediaAssetReadPort` / `media.asset_read.v1` source-locked FBA evidence, including deadline/context guards, typed `PortError` retryability mapping, and `MediaAssetSummary` kind/usage metadata for consumers.
- Publish `MediaAssetWritePort` / `media.asset_write.v1` for upload preparation/completion, deletion, translations, and tenant-scoped reconciliation. Binary bodies never enter generic write-port DTOs.
- Publish `MediaPublicImageReadPort` for embedded public presentation. It returns the canonical `MediaItem` plus a Media-issued descriptor: direct-public URLs remain unchanged, storage-relative image paths become immutable capability URLs, and opaque references remain unavailable.
- Serve capability URLs at `/api/media/public/images/{id}/{checksum_sha256}`. The handler verifies tenant, active asset/blob, ready state, image MIME, active-blob SHA-256, and object size before returning bytes with immutable cache headers and ETag.

## Interactions

- Depends on `rustok-core` for shared runtime helpers such as `generate_id()`.
- Depends on `object_store` directly for blob operations and on `rustok-storage` only for runtime construction, delivery configuration, and key policy.
- Depends on `rustok-api` for shared tenant/auth and port contracts.
- Depends on `rustok-translation-targets` for the neutral provider SPI and on
  `rustok-outbox` for atomic owner change evidence plus the generic durable
  receipt primitive; translation workflow state remains outside Media.
- Exposes its own GraphQL and REST adapters; `apps/server` acts only as a composition root and re-export shim for media transport entry points.
- Exposes `mediaUsage` from the owner `MediaQuery`; `apps/server` only composes the module query.
- Media-library REST adapters require authenticated `AuthContext`; the public-image capability GET is intentionally unauthenticated and derives tenant authority from `TenantContext`, while an invalid id/checksum/tenant/lifecycle/MIME combination is indistinguishable as not found.
- `rustok-seo`, Profiles, and owner SEO providers consume Media-owned image descriptors without raw media blob coupling.
- Profiles revalidates tenant, uploader, and image MIME on the `MediaItem` returned alongside the descriptor; it never constructs the capability path.
- `rustok-media-admin` uses native Leptos `#[server]` functions as the default internal data layer, keeps GraphQL as the selected path for list/detail/translations/delete/usage, and preserves REST upload via `/api/media`.
- Media is the whole-module remote extraction pilot. `rustok-media-transport` supplies gRPC adapters for the existing metadata/control ports. The new public-image capability is currently an embedded DB/storage owner surface; remote provider parity and deployment evidence remain required before it can participate in an extracted Media provider.

## Entry points

- `MediaService`
- `MediaTranslationTargetProvider`
- `MediaPublicImageService` / `MediaPublicImageReadPort`
- `MediaHttpRuntime`
- `load_media_usage_snapshot`
- `graphql::MediaQuery` / `graphql::MediaMutation`
- `controllers::axum_router`
- `rustok-media-admin`
- `MediaReconciliationDecision` / `MediaReconciliationReport`
- `rustok-media-cli` (`media reconcile [--limit <count>]`)
- `MediaAssetSummary` / `MediaAssetKind` / `MediaAssetUsageProfile`
- `MediaAssetReadPort` / `MediaAssetWritePort`
- `MediaUploadRequest` / `MediaUploadTarget`
- `rustok-media-transport::{GrpcMediaProvider, MediaGrpcService}`
- `CreateRenditionInput` / `MediaRenditionItem` / `ImageWorker`
- `PrepareUploadSessionInput` / `PreparedUploadSession`
- `MediaImageDescriptor` / `MediaImageDeliveryProfile` / `MediaImagePublicUrlPolicy`
- `MediaItem` / `MediaTranslationItem`
- `UploadInput` / `UpsertTranslationInput` / `NormalizedTranslationInput`

## Runtime notes

- Translation upserts convert external locale strings to
  `rustok_api::TenantLocale` before owner persistence, preserving canonical
  BCP47 casing and rejecting storage-only `und`; blank optional text fields
  become `None`, and translation lists are returned in locale order.
- `apply_exact_translation` locks the asset and ordered source/target locale
  rows in one owner transaction, checks both expected revisions, and advances
  only the exact target revision.
- Provider apply admits a `media`-scoped shared receipt before the owner write,
  then commits its result with the Media mutation and change evidence in the
  same transaction; the generic ledger never owns Media lifecycle semantics.
- Change cursors are ordered owner-generated identifiers. Every non-empty
  `read_changes` page returns the last consumed identifier as its checkpoint;
  replaying a provider idempotency key does not append another change or event.
- Aggregate progress counts only exact target-row values for source-eligible
  active assets. It brackets count queries with the owner cursor and retries a
  changing observation instead of returning internally inconsistent facts.
- Reconciliation prioritizes delete tombstones, rotates ready blobs through persisted progress, and preserves owner-local lifecycle evidence. Missing rendition output is isolated from a healthy source asset.
- Upload-session reconciliation removes completed or expired staging objects and preserves retryable failures. Repeating finalization returns the asset already bound to the session.
- Public-image capability URLs bind the stable asset id to the active blob checksum. Changing the active blob changes the URL; deleting or failing the asset/blob makes the old URL unavailable.
- The public image handler returns `Cache-Control: public, max-age=31536000, immutable`, a checksum ETag, `nosniff`, and the owner MIME/length. It never exposes storage keys in the URL or an error body.
- Port calls require deadline semantics, UUID tenant context, non-retryable domain validation/access errors, and retryable unavailable errors for storage/database failures.

## Verification

- `cargo test -p rustok-media --test public_image_proxy -- --nocapture`
- `node scripts/verify/verify-media-public-image-proxy.mjs`
- existing Media module, lifecycle, FBA, admin-boundary, Local, and S3 verification commands remain maintainer-run.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Media and Search extraction ADR](../../DECISIONS/2026-07-16-media-search-extraction-boundaries.md)
- [Platform docs index](../../docs/index.md)
