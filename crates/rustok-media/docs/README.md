# Documentation `rustok-media`

`rustok-media` is the domain owner and metadata index for media asset
management on the platform. It handles images, video and PDF assets while
calling the host-provided `object_store` runtime directly; `rustok-storage`
only constructs that runtime and enforces canonical keys.

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
  preparation/completion, delete, translation, and tenant-scoped reconciliation; binary bytes stay
  on Media-owned streaming REST or short-lived presigned S3 PUT targets, outside generic port DTOs;
- loopback-verified `rustok-media-transport` tonic adapters for all read/write
  control operations; gRPC propagates deadlines and exact typed owner errors
  while binary bodies remain outside the service. Remote calls require a
  server-side `TrustedMediaAuthority` extension; caller-supplied tenant and
  principal fields are never authoritative;
- GraphQL and REST adapters of the module;
- upload validation by size/MIME policy and tenant isolation before accessing storage;
- module-owned admin UI package `rustok-media-admin` with FFA split `core`/`transport`/`ui/leptos`; native server functions use `HostRuntimeContext` and the host-provided `StorageRuntime` instead of a host-wide `AppContext`;
- observability signals for upload, delete, rendition latency/outcome, upload sessions,
  reconciliation outcomes, and storage health;
- translation normalization: `locale` trim/lowercase, empty `title`/`alt_text`/`caption` are stored as `None`, translation lists are returned in stable order by locale;
- owner-local lifecycle persistence in `media_assets`, `media_blobs`, `media_renditions`, `media_upload_sessions`, `media_translations`, and durable `media_port_operations`; the former Content-owned `media` migration no longer exists.
- reconciliation contract: `reconcile_storage` probes exact immutable object keys with a rotating persisted cursor, marks a missing active blob as failed without deleting evidence, isolates missing rendition results, retries transient failures, completes persisted delete tombstones, and removes only eligible staging objects; `MediaReconciliationReport` exposes healthy, missing, deletion, and retry counts.
- `rustok-media-cli` provides `media reconcile`; it explicitly builds `StorageRuntime` from the host-neutral CLI storage settings and invokes the Media service across tenants.
- the image pipeline emits immutable JPEG, PNG, WebP, and AVIF renditions with golden-output,
  orientation, animated-input rejection, memory, timeout, and concurrency tests;
- Local and env-gated S3-compatible integration tests exercise the same Media lifecycle;

## Integration

- uses the host-provided direct `object_store` runtime; Media rows keep immutable object
  references and lifecycle metadata, never a backend/driver name;
- `apps/server` remains the composition root and wiring layer for media routes/graphql;
- runtime guard relies on tenant-scoped module enablement for public surfaces;
- upload remains REST-owned, GraphQL is preserved for read/mutation flows without multipart extension, and the Leptos admin adapter calls the transport facade instead of the raw API module; the transport facade inside the admin package splits native server functions, the GraphQL selected path and REST upload adapters, while upload/detail presentation state remains in Leptos-free `admin/src/core.rs`;
- `rustok-seo` and owner SEO providers consume `MediaImageDescriptor` as the sole image boundary for OG/Twitter/schema fallback; descriptor normalization covers explicit MIME, dropping invalid sizes, cleaning query/fragment, delivery profile classification and public URL policy for storage-relative paths requiring a proxy;
- `MediaAssetReadPort` requires deadline semantics, UUID tenant context and returns typed `PortError`: validation/access/not-found errors are non-retryable, while storage/database failures are returned as retryable unavailable; descriptor consumers must not directly publish storage-relative paths in public metadata and must route `ProxyRequired` descriptors through the host proxy.

## Verification

- `cargo xtask module validate media`
- `cargo xtask module test media`
- `cargo test -p rustok-media-transport`
- targeted tests for upload validation, translation normalization, reconciliation classification, local object lifecycle and admin-facing read/write contracts

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Admin package](../admin/README.md)
- [gRPC transport](../../rustok-media-transport/docs/README.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)

## Host boundary notes

- `load_media_usage_snapshot` remains an owner service API for usage statistics.
- The GraphQL field `mediaUsage` and DTO `MediaUsageStats` belong to `rustok-media::graphql::MediaQuery`;
  `apps/server::SystemQuery` does not import the media API and only participates in the overall schema composition.
