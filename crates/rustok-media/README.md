# rustok-media

## Purpose

`rustok-media` owns media asset uploads, metadata, translations, and transport adapters for RusToK.

## Responsibilities

- Provide the shared media domain service and SeaORM entities for uploads and localized metadata with normalized locale/text translation inputs.
- Own media GraphQL and REST transport adapters for module-facing APIs.
- Keep REST upload/list/get/delete/translation handlers on narrow `MediaHttpRuntime` state; the manifest-declared Axum router builds it from `HostRuntimeContext` and a typed storage handle.
- Publish the module-owned Leptos admin UI crate `rustok-media-admin`.
- Own storage-backed media lifecycle state while calling the shared `object_store` runtime directly.
- Generate immutable source and rendition keys through the canonical tenant/date/shard policy.
- Own validated image edit recipes and bounded pure-Rust processing.
- Publish the module-local `rustok-media-cli` adapter with `media reconcile`, keeping CLI/runtime assembly outside the domain crate.
- Expose `MediaImageDescriptor` as the typed cross-module image contract (`url/alt/size/mime` + derived helpers, delivery profile, public URL policy, and proxy path helper) for SEO and other read-side consumers.
- Publish `MediaAssetReadPort` / `media.asset_read.v1` source-locked FBA evidence, including deadline/context guards, typed `PortError` retryability mapping, and `MediaAssetSummary` kind/usage metadata for consumers.
- Publish `MediaAssetWritePort` / `media.asset_write.v1` for upload preparation/completion,
  deletion, translations, and tenant-scoped reconciliation. Local uploads use the Media-owned
  streaming REST target; S3-compatible runtimes issue short-lived presigned PUT sessions.
  Binary bodies never enter generic port DTOs.

## Interactions

- Depends on `rustok-core` for shared runtime helpers such as `generate_id()`.
- Depends on `object_store` directly for blob operations and on `rustok-storage` only for runtime construction, delivery configuration, and key policy.
- Depends on `rustok-api` for shared tenant/auth and GraphQL helper contracts.
- Exposes its own GraphQL and REST adapters; `apps/server` now acts only as a composition root
  and re-export shim for media transport entry points.
- Exposes `mediaUsage` from the owner `MediaQuery`; `apps/server` only composes the module query.
- REST adapters require authenticated `AuthContext`; GraphQL resolvers keep the existing
  module-enabled guard and tenant-explicit contract.
- `rustok-seo` and owner SEO providers consume `MediaImageDescriptor` to build OG/Twitter/schema
  fallback surfaces without raw media blob coupling.
- `rustok-media-admin` uses native Leptos `#[server]` functions as the default internal data layer,
  keeps GraphQL as the selected path for `list/detail/translations/delete/usage`, and preserves REST primary
  upload via `/api/media`.
- Media is the first whole-module remote extraction pilot. The modular
  monolith uses the embedded provider; `rustok-media-transport` supplies a
  loopback-verified gRPC provider with the same owner service, DTO, deadline,
  typed-error, database/schema, storage-credential, and port semantics.

## Entry points

- `MediaService`
- `MediaHttpRuntime`
- `load_media_usage_snapshot`
- `graphql::MediaQuery` (`mediaUsage`, media list/detail/translations)
- `graphql::MediaMutation`
- `controllers::axum_router`
- `rustok-media-admin`
- `MediaReconciliationDecision` / `MediaReconciliationReport`
- `rustok-media-cli` (`media reconcile [--limit <count>]`)
- `MediaAssetSummary` / `MediaAssetKind` / `MediaAssetUsageProfile`
- `MediaAssetWritePort` / `MediaUploadRequest` / `MediaUploadTarget`
- `rustok-media-transport::{GrpcMediaProvider, MediaGrpcService}`
- `CreateRenditionInput` / `MediaRenditionItem` / `ImageWorker`
- `PrepareUploadSessionInput` / `PreparedUploadSession`
- `MediaImageDescriptor` / `MediaImageDeliveryProfile` / `MediaImagePublicUrlPolicy`
- `MediaItem`
- `MediaTranslationItem`
- `UploadInput`
- `UpsertTranslationInput` / `NormalizedTranslationInput`

## Runtime notes

- Translation upserts normalize locale and text payloads before persistence: locale values are trimmed/lowercased, blank optional text fields become `None`, and translation lists are returned in locale order.
- Reconciliation uses object metadata probes and preserves owner-local lifecycle evidence; a missing object must transition lifecycle state instead of silently erasing the asset record.
- Upload-session reconciliation removes completed or expired staging objects and preserves
  retryable failures. Repeating finalization returns the asset already bound to the session.
- `media reconcile` creates storage explicitly from the CLI's `storage` settings snapshot and requires the CLI database runtime. Its limit is global across tenants, so it bounds one maintenance invocation without changing owner-local lifecycle policy.
- FBA provider calls require non-zero `PortContext.deadline_ms`, UUID tenant context, non-retryable domain validation/access errors, and retryable unavailable errors for storage/database failures. Descriptor consumers must emit only direct public URLs into public metadata; storage-relative descriptors are explicitly marked `ProxyRequired` and can derive a host proxy path with `MediaImageDescriptor::proxy_path`. `MediaAssetSummary` classifies media by MIME kind and usage profile without exposing raw blobs.

## Docs

- [Module docs](./docs/README.md)
- [Media and Search extraction ADR](../../DECISIONS/2026-07-16-media-search-extraction-boundaries.md)
- [Platform docs index](../../docs/index.md)
