# Documentation `rustok-media`

`rustok-media` is the domain facade and metadata index for media asset
management on the platform. It handles images, video and PDF assets while
relying on `rustok-storage` as the physical binary/object owner.

## Purpose

- publish the canonical runtime media contract for upload, list, delete and translation scenarios;
- keep media metadata, classification, validation and transport surfaces inside the module;
- provide a platform media capability without diluting domain logic across the host layer.

## Scope

- `MediaService`, media entities/DTOs and the translation update contract with locale/text normalization at the runtime boundary;
- REST upload/list/get/delete/translation handlers on a narrow `MediaHttpRuntime` with explicit DB/storage handles; `controllers::axum_router` builds it from `HostRuntimeContext` and generated host composition mounts it without a framework adapter;
- typed cross-module image contract `MediaImageDescriptor` (`url/alt/size/mime` + derived helpers), `MediaImageDeliveryProfile`, `MediaImagePublicUrlPolicy` and `proxy_path` helper for explicit direct-public/proxy-required/not-addressable URL policy;
- FBA provider contract `MediaAssetReadPort` / `media.asset_read.v1` with source-locked evidence for deadline/context guards, typed `PortError` retryability and `MediaAssetSummary` kind/usage metadata;
- FBA owner control contract `MediaAssetWritePort` / `media.asset_write.v1` for upload target
  preparation, delete, translation, and tenant-scoped cleanup; binary bytes stay on Media-owned
  streaming REST or a future presigned upload target, outside generic port DTOs;
- GraphQL and REST adapters of the module;
- upload validation by size/MIME policy and tenant isolation before accessing storage;
- module-owned admin UI package `rustok-media-admin` with FFA split `core`/`transport`/`ui/leptos`; native server functions use `HostRuntimeContext` and host-provided typed `StorageService` instead of a host-wide `AppContext`;
- observability signals for upload, delete and storage health;
- translation normalization: `locale` trim/lowercase, empty `title`/`alt_text`/`caption` are stored as `None`, translation lists are returned in stable order by locale;
- conservative cleanup contract: `cleanup_storage_orphans` reads exact `storage_path`, does not delete readable objects, removes only DB rows for `NotFound`/`InvalidPath`, and treats `Io`/`Backend` as retryable failures; `MediaStorageCleanupReport` publishes helpers for empty/change/retry state.
- `rustok-media-cli` provides `media cleanup [--limit <count>]`; it explicitly builds `StorageService` from the host-neutral CLI `storage` settings snapshot and invokes the owner service across tenants. The default limit is 1,000 records.

## Integration

- uses `rustok-storage` as the physical backend storage contract; media records
  keep object references and media metadata, not backend ownership;
- `apps/server` remains the composition root and wiring layer for media routes/graphql;
- runtime guard relies on tenant-scoped module enablement for public surfaces;
- upload remains REST-owned, GraphQL is preserved for read/mutation flows without multipart extension, and the Leptos admin adapter calls the transport facade instead of the raw API module; the transport facade inside the admin package splits native server functions, the GraphQL selected path and REST upload adapters, while upload/detail presentation state remains in Leptos-free `admin/src/core.rs`;
- `rustok-seo` and owner SEO providers consume `MediaImageDescriptor` as the sole image boundary for OG/Twitter/schema fallback; descriptor normalization covers explicit MIME, dropping invalid sizes, cleaning query/fragment, delivery profile classification and public URL policy for storage-relative paths requiring a proxy;
- `MediaAssetReadPort` requires deadline semantics, UUID tenant context and returns typed `PortError`: validation/access/not-found errors are non-retryable, while storage/database failures are returned as retryable unavailable; descriptor consumers must not directly publish storage-relative paths in public metadata and must route `ProxyRequired` descriptors through the host proxy.

## Verification

- `cargo xtask module validate media`
- `cargo xtask module test media`
- targeted tests for upload validation, translation normalization, cleanup probe classification, storage cleanup and admin-facing read/write contracts

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Admin package](../admin/README.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)

## Host boundary notes

- `load_media_usage_snapshot` remains an owner service API for usage statistics.
- The GraphQL field `mediaUsage` and DTO `MediaUsageStats` belong to `rustok-media::graphql::MediaQuery`;
  `apps/server::SystemQuery` does not import the media API and only participates in the overall schema composition.
