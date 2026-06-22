# rustok-media

## Purpose

`rustok-media` owns media asset uploads, metadata, translations, and transport adapters for RusToK.

## Responsibilities

- Provide the shared media domain service and SeaORM entities for uploads and localized metadata with normalized locale/text translation inputs.
- Own media GraphQL and REST transport adapters for module-facing APIs.
- Publish the module-owned Leptos admin UI crate `rustok-media-admin`.
- Integrate storage-backed file lifecycle with tenant-aware media records, including conservative cleanup probes and reports that never delete readable storage objects during orphan detection.
- Expose `MediaImageDescriptor` as the typed cross-module image contract (`url/alt/size/mime` + derived helpers, delivery profile, and public URL policy) for SEO and other read-side consumers.
- Publish `MediaAssetReadPort` / `media.asset_read.v1` source-locked FBA evidence, including deadline/context guards and typed `PortError` retryability mapping for consumers.

## Interactions

- Depends on `rustok-core` for shared runtime helpers such as `generate_id()`.
- Depends on `rustok-storage` for blob persistence and public URL resolution.
- Depends on `rustok-api` for shared tenant/auth and GraphQL helper contracts.
- Exposes its own GraphQL and REST adapters; `apps/server` now acts only as a composition root
  and re-export shim for media transport entry points.
- REST adapters require authenticated `AuthContext`; GraphQL resolvers keep the existing
  module-enabled guard and tenant-explicit contract.
- `rustok-seo` and owner SEO providers consume `MediaImageDescriptor` to build OG/Twitter/schema
  fallback surfaces without raw media blob coupling.
- `rustok-media-admin` uses native Leptos `#[server]` functions as the default internal data layer,
  keeps GraphQL as the fallback for `list/detail/translations/delete/usage`, and preserves REST-first
  upload via `/api/media`.

## Entry points

- `MediaService`
- `graphql::MediaQuery`
- `graphql::MediaMutation`
- `controllers::routes`
- `rustok-media-admin`
- `MediaStorageCleanupDecision` / `MediaStorageCleanupReport`
- `MediaImageDescriptor` / `MediaImageDeliveryProfile` / `MediaImagePublicUrlPolicy`
- `MediaItem`
- `MediaTranslationItem`
- `UploadInput`
- `UpsertTranslationInput` / `NormalizedTranslationInput`

## Runtime notes

- Translation upserts normalize locale and text payloads before persistence: locale values are trimmed/lowercased, blank optional text fields become `None`, and translation lists are returned in locale order.
- Server cleanup uses a read probe for the exact `storage_path`; readable objects keep their DB record, `NotFound`/`InvalidPath` remove only the DB record, and transient `Io`/`Backend` errors keep the record for a later retry.
- FBA provider calls require non-zero `PortContext.deadline_ms`, UUID tenant context, non-retryable domain validation/access errors, and retryable unavailable errors for storage/database failures. Descriptor consumers must emit only direct public URLs into public metadata; storage-relative descriptors are explicitly marked `ProxyRequired`.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
